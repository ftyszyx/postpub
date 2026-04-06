use std::sync::Arc;

use chrono::Utc;
use postpub_types::{
    ArticleVariantDocument, ArticleVariantSummary, CustomLlmProvider, GenerateArticleRequest,
    GenerationOutput, PostpubConfig, PublishTargetConfig, SearchResult, TemplateDocument, UiConfig,
};
use regex::Regex;
use serde::Deserialize;

use crate::{
    aiforge::AiforgeEngine,
    articles::{markdown_to_html, preview_html, ArticleStore},
    error::{PostpubError, Result},
    templates::TemplateStore,
    AppContext,
};

#[derive(Debug, Clone)]
pub struct GenerationService {
    context: AppContext,
}

#[derive(Clone, Default)]
pub struct GenerationProgressReporter {
    callback: Option<Arc<dyn Fn(String, String) + Send + Sync>>,
}

impl GenerationProgressReporter {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(String, String) + Send + Sync + 'static,
    {
        Self {
            callback: Some(Arc::new(callback)),
        }
    }

    pub fn report(&self, stage: impl Into<String>, message: impl Into<String>) {
        if let Some(callback) = &self.callback {
            callback(stage.into(), message.into());
        }
    }
}

impl GenerationService {
    pub fn new(context: AppContext) -> Self {
        Self { context }
    }

    pub async fn generate(&self, request: GenerateArticleRequest) -> Result<GenerationOutput> {
        self.generate_with_progress(request, GenerationProgressReporter::default())
            .await
    }

    pub async fn generate_with_progress(
        &self,
        request: GenerateArticleRequest,
        progress: GenerationProgressReporter,
    ) -> Result<GenerationOutput> {
        progress.report("prepare", "正在加载生成配置");
        let bundle = self.context.config_store().load_bundle()?;
        let config = bundle.config.clone();
        let template_store = TemplateStore::new(self.context.paths().clone());
        let provider = select_generation_provider(&bundle.ui_config)?;
        progress.report(
            "provider",
            format!(
                "正在校验 LLM 提供商 '{}'",
                provider_display_name(&provider)
            ),
        );
        let api_key = resolve_provider_api_key(&provider)?;

        progress.report(
            "provider",
            format!(
                "已选择 LLM 提供商 '{}'，模型 '{}'",
                provider_display_name(&provider),
                provider.model
            ),
        );

        let engine = AiforgeEngine::new(self.context.http_client().clone(), bundle.aiforge_config);
        let (mode, sources) = if request.reference_urls.is_empty() {
            progress.report(
                "retrieval",
                "未提供参考链接，跳过联网搜索，直接按主题生成",
            );
            ("topic".to_string(), Vec::new())
        } else {
            progress.report(
                "retrieval",
                "正在抓取参考链接内容",
            );
            let (mode, sources) = engine
                .collect_sources(
                    &request.topic,
                    &request.reference_urls,
                    config.aiforge_search_min_results,
                    config.aiforge_search_max_results,
                )
                .await?;
            progress.report(
                "retrieval",
                format!("已收集 {} 条来源，模式为 '{}'", sources.len(), mode),
            );
            (mode, sources)
        };

        let title = build_title(&request);
        progress.report("draft", format!("正在生成 Markdown 初稿《{title}》"));
        let markdown = generate_markdown_with_llm(
            self.context.http_client(),
            &provider,
            &api_key,
            &title,
            &request,
            &config,
            &mode,
            &sources,
        )
        .await?;
        progress.report("draft", "Markdown 初稿已生成");

        progress.report("variant", "正在生成发布稿变体");
        let variants = build_publish_variants(
            self.context.http_client(),
            &template_store,
            &request,
            &config,
            &title,
            &markdown,
            &provider,
            &api_key,
            &progress,
        )
        .await?;
        progress.report(
            "variant",
            format!("已生成 {} 个发布稿变体", variants.len()),
        );

        let article = if request.save_output {
            progress.report("save", "正在保存源文章");
            Some(
                ArticleStore::new(self.context.paths().clone())
                    .save_generated_source_article(&title, &markdown, &variants)?
                    .summary,
            )
        } else {
            None
        };

        if article.is_some() {
            progress.report("save", "源文章已保存");
        }

        Ok(GenerationOutput {
            title,
            format: "MD".to_string(),
            content: markdown.clone(),
            preview_html: markdown_to_html(&markdown),
            variants,
            mode,
            sources,
            article,
        })
    }
}

async fn build_publish_variants(
    client: &reqwest::Client,
    template_store: &TemplateStore,
    request: &GenerateArticleRequest,
    config: &PostpubConfig,
    title: &str,
    markdown: &str,
    provider: &CustomLlmProvider,
    api_key: &str,
    progress: &GenerationProgressReporter,
) -> Result<Vec<ArticleVariantDocument>> {
    let now = Utc::now();
    let mut variants = Vec::new();

    for target in config
        .publish_targets
        .iter()
        .filter(|target| target.enabled)
    {
        let format = normalize_format(&target.article_format);
        let selected_template = load_variant_template(template_store, request, config, target)?;

        progress.report(
            "variant",
            format!(
                "正在为目标 '{}' 生成 {} 格式内容",
                target.name,
                format.to_uppercase(),
            ),
        );

        let content = match format.as_str() {
            "html" => {
                generate_html_with_llm(
                    client,
                    provider,
                    api_key,
                    &target.platform_type,
                    target.min_article_len,
                    target.max_article_len,
                    title,
                    markdown,
                    selected_template.as_ref(),
                )
                .await?
            }
            "md" => markdown.to_string(),
            _ => markdown_to_plain_text(markdown),
        };
        let preview = preview_html(&format, &content);

        variants.push(ArticleVariantDocument {
            summary: ArticleVariantSummary {
                target_id: target.id.clone(),
                target_name: target.name.clone(),
                platform_type: target.platform_type.clone(),
                format: format.to_uppercase(),
                size_bytes: content.len() as u64,
                updated_at: now,
            },
            content,
            preview_html: preview,
        });

        progress.report(
            "variant",
            format!(
                "目标 '{}' 的 {} 格式内容已生成",
                target.name,
                format.to_uppercase()
            ),
        );
    }

    Ok(variants)
}

fn load_variant_template(
    template_store: &TemplateStore,
    request: &GenerateArticleRequest,
    config: &PostpubConfig,
    target: &PublishTargetConfig,
) -> Result<Option<TemplateDocument>> {
    if !target.use_template {
        return Ok(None);
    }

    template_store.load_selected_template(
        non_empty(target.template_category.as_str())
            .or(request.template_category.as_deref())
            .or(non_empty(config.template_category.as_str())),
        non_empty(target.template_name.as_str())
            .or(request.template_name.as_deref())
            .or(non_empty(config.template_name.as_str())),
    )
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

async fn generate_markdown_with_llm(
    client: &reqwest::Client,
    provider: &CustomLlmProvider,
    api_key: &str,
    title: &str,
    request: &GenerateArticleRequest,
    config: &PostpubConfig,
    mode: &str,
    sources: &[SearchResult],
) -> Result<String> {
    let raw_markdown = execute_chat_completion(
        client,
        provider,
        api_key,
        "You are a professional Chinese content writer. Produce only the final Markdown article.",
        &build_writer_prompt(title, request, config, mode, sources),
    )
    .await?;

    Ok(normalize_llm_markdown(&raw_markdown, title))
}

async fn generate_html_with_llm(
    client: &reqwest::Client,
    provider: &CustomLlmProvider,
    api_key: &str,
    publish_platform: &str,
    min_article_len: usize,
    max_article_len: usize,
    title: &str,
    markdown: &str,
    template: Option<&TemplateDocument>,
) -> Result<String> {
    let (system_prompt, user_prompt) = if let Some(template) = template {
        (
            build_template_system_prompt(publish_platform),
            build_template_user_prompt(
                publish_platform,
                markdown,
                title,
                &template.content,
                min_article_len,
                max_article_len,
            ),
        )
    } else {
        (
            build_design_system_prompt(publish_platform),
            build_design_user_prompt(publish_platform, markdown, title),
        )
    };

    let raw_html =
        execute_chat_completion(client, provider, api_key, &system_prompt, &user_prompt).await?;
    Ok(normalize_llm_html(&raw_html)?)
}

async fn execute_chat_completion(
    client: &reqwest::Client,
    provider: &CustomLlmProvider,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String> {
    let endpoint = chat_completions_endpoint(&provider.api_base, &provider.protocol_type)?;
    let response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": provider.model.as_str(),
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt,
                },
                {
                    "role": "user",
                    "content": user_prompt,
                }
            ],
            "max_tokens": provider.max_tokens,
            "stream": false
        }))
        .send()
        .await?
        .error_for_status()?;

    let payload = response.json::<ChatCompletionsResponse>().await?;
    payload.first_text().ok_or_else(|| {
        PostpubError::External("llm response did not contain a text message".to_string())
    })
}

fn build_title(request: &GenerateArticleRequest) -> String {
    request.topic.trim().to_string()
}

fn build_writer_prompt(
    title: &str,
    request: &GenerateArticleRequest,
    config: &PostpubConfig,
    mode: &str,
    sources: &[SearchResult],
) -> String {
    let (min_article_len, max_article_len) = source_article_length_range(config);
    let mode_instructions = match mode {
        "reference" => concat!(
            "1. Use the source material below as the factual basis.\n",
            "2. Preserve useful structure and viewpoints from the reference articles without copying verbatim.\n",
            "3. Use only dates, data, and claims that are supported by the sources.\n",
            "4. Output only the final Markdown article. Do not include analysis or notes."
        ),
        "topic" => concat!(
            "1. Generate directly from the topic without using external web search.\n",
            "2. Keep the article grounded in the provided topic and broadly reliable common knowledge.\n",
            "3. If a precise date, number, or claim cannot be supported, rewrite it in a general way instead of inventing details.\n",
            "4. Output only the final Markdown article. Do not include analysis or notes."
        ),
        _ => concat!(
            "1. Use the source material below as the factual basis.\n",
            "2. Synthesize the search results into an original article.\n",
            "3. Use only dates, data, and claims that are supported by the sources.\n",
            "4. Output only the final Markdown article. Do not include analysis or notes."
        ),
    };

    format!(
        "Task: write a high-quality Markdown article in Simplified Chinese.\n\
Topic: {topic}\n\
Mode: {mode}\n\
Required title: # {title}\n\
Target length: {min_len}-{max_len} Chinese characters\n\
\n\
Instructions:\n\
{mode_instructions}\n\
\n\
Source material:\n{source_context}",
        topic = request.topic.trim(),
        mode = mode,
        title = title,
        min_len = min_article_len,
        max_len = max_article_len,
        mode_instructions = mode_instructions,
        source_context = format_sources_for_prompt(request, mode, sources),
    )
}

fn build_template_system_prompt(publish_platform: &str) -> String {
    format!(
        "You adapt article content into HTML for the '{}' publishing platform. Keep the result valid HTML only.",
        publish_platform
    )
}

fn build_template_user_prompt(
    publish_platform: &str,
    markdown: &str,
    title: &str,
    template_content: &str,
    min_article_len: usize,
    max_article_len: usize,
) -> String {
    format!(
        "Adapt the Markdown article into the provided HTML template for platform '{publish_platform}'.\n\
Preserve the template's overall structure and style as much as possible.\n\
Article title: {title}\n\
Target length: {min_len}-{max_len}\n\
\n\
Markdown article:\n{markdown}\n\
\n\
HTML template:\n{template_content}\n\
\n\
Return HTML only.",
        publish_platform = publish_platform,
        title = title,
        min_len = min_article_len,
        max_len = max_article_len,
        markdown = markdown,
        template_content = template_content,
    )
}

fn build_design_system_prompt(publish_platform: &str) -> String {
    format!(
        "You design clean publication-ready HTML for the '{}' platform. Return HTML only.",
        publish_platform
    )
}

fn build_design_user_prompt(publish_platform: &str, markdown: &str, title: &str) -> String {
    format!(
        "Create polished HTML for platform '{publish_platform}'.\n\
Title: {title}\n\
\n\
Markdown article:\n{markdown}\n\
\n\
Return HTML only.",
        publish_platform = publish_platform,
        title = title,
        markdown = markdown,
    )
}

fn format_sources_for_prompt(
    request: &GenerateArticleRequest,
    mode: &str,
    sources: &[SearchResult],
) -> String {
    if mode == "topic" {
        return format!(
            "No external sources were provided for '{topic}'. Generate directly from the topic and avoid fabricating precise facts.",
            topic = request.topic.trim(),
        );
    }

    if sources.is_empty() {
        return format!(
            "No {source_type} sources were collected for '{topic}'.",
            source_type = source_type_label(mode, request),
            topic = request.topic.trim(),
        );
    }

    let mut formatted = format!(
        "{source_type} sources for '{topic}':\n\n",
        source_type = source_type_label(mode, request),
        topic = request.topic.trim(),
    );

    for (index, source) in sources.iter().enumerate() {
        let title = truncate_chars_with_ellipsis(source.title.trim(), 100);
        let summary_source = if !source.abstract_text.trim().is_empty() {
            source.abstract_text.trim()
        } else {
            source.content.as_deref().unwrap_or("").trim()
        };
        let summary = truncate_chars_with_ellipsis(summary_source, 300);

        formatted.push_str(&format!("## Result {}\n", index + 1));
        formatted.push_str(&format!(
            "Title: {}\n",
            if title.is_empty() {
                "Untitled"
            } else {
                title.as_str()
            }
        ));
        formatted.push_str(&format!(
            "Published at: {}\n",
            source.published_at.as_deref().unwrap_or("Unknown")
        ));
        formatted.push_str(&format!(
            "Summary: {}\n",
            if summary.is_empty() {
                "No summary"
            } else {
                summary.as_str()
            }
        ));
        formatted.push_str(&format!("URL: {}\n", source.url));

        if !request.reference_urls.is_empty() || mode == "reference" {
            let content =
                truncate_chars_with_ellipsis(source.content.as_deref().unwrap_or("").trim(), 500);
            if !content.is_empty() {
                formatted.push_str(&format!("Content: {}\n", content));
            }
        }

        formatted.push('\n');
    }

    formatted
}

fn source_type_label<'a>(mode: &'a str, request: &'a GenerateArticleRequest) -> &'a str {
    if mode == "topic" {
        "topic"
    } else if !request.reference_urls.is_empty() || mode == "reference" {
        "reference"
    } else {
        "search"
    }
}

fn truncate_chars_with_ellipsis(text: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= max_chars {
            output.push_str("...");
            break;
        }
        output.push(ch);
    }
    output
}

fn select_generation_provider(ui_config: &UiConfig) -> Result<CustomLlmProvider> {
    ui_config
        .custom_llm_providers
        .iter()
        .find(|provider| {
            provider.enabled
                && !provider.model.trim().is_empty()
                && !provider.api_base.trim().is_empty()
                && matches!(
                    provider.protocol_type.trim(),
                    "openai" | "openai_compatible" | "ollama" | "custom"
                )
        })
        .cloned()
        .ok_or_else(|| {
            PostpubError::Validation(
                "no enabled LLM provider is available; configure one in Settings > LLM models"
                    .to_string(),
            )
        })
}

fn provider_display_name(provider: &CustomLlmProvider) -> &str {
    let name = provider.name.trim();
    if name.is_empty() {
        provider.model.trim()
    } else {
        name
    }
}

fn resolve_provider_api_key(provider: &CustomLlmProvider) -> Result<String> {
    let inline_key = provider.api_key.trim();
    if !inline_key.is_empty() {
        return Ok(inline_key.to_string());
    }

    Err(PostpubError::Validation(format!(
        "LLM provider '{}' is missing an API key; fill api_key in Settings > LLM models",
        provider_display_name(provider)
    )))
}

fn chat_completions_endpoint(api_base: &str, protocol_type: &str) -> Result<String> {
    if !matches!(
        protocol_type.trim(),
        "openai" | "openai_compatible" | "ollama" | "custom"
    ) {
        return Err(PostpubError::Validation(format!(
            "unsupported llm protocol type: {}",
            protocol_type.trim()
        )));
    }

    let trimmed = api_base.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(PostpubError::Validation(
            "llm provider api_base is required".to_string(),
        ));
    }

    if trimmed.ends_with("/chat/completions") {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{trimmed}/chat/completions"))
    }
}

fn normalize_llm_markdown(raw: &str, title: &str) -> String {
    let without_fences = strip_code_fences(raw);
    let content = without_fences.trim();

    if content.is_empty() {
        return format!("# {title}");
    }

    let mut lines = content.lines();
    let Some(first_non_empty) = lines.find(|line| !line.trim().is_empty()) else {
        return format!("# {title}");
    };

    let rebuilt = if first_non_empty.trim_start().starts_with("# ") {
        let mut output = Vec::new();
        let mut replaced = false;
        for line in content.lines() {
            if !replaced && !line.trim().is_empty() && line.trim_start().starts_with("# ") {
                output.push(format!("# {title}"));
                replaced = true;
            } else {
                output.push(line.to_string());
            }
        }
        output.join("\n")
    } else {
        format!("# {title}\n\n{content}")
    };

    rebuilt.trim().to_string()
}

fn normalize_llm_html(raw: &str) -> Result<String> {
    let normalized = strip_code_fences(raw);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Err(PostpubError::External(
            "llm html response was empty".to_string(),
        ));
    }

    if trimmed.contains('<') && trimmed.contains('>') {
        Ok(trimmed.to_string())
    } else {
        Err(PostpubError::External(
            "llm html response did not contain markup".to_string(),
        ))
    }
}

fn strip_code_fences(raw: &str) -> String {
    let trimmed = raw.trim();
    let fenced_block =
        Regex::new(r"(?s)```[a-zA-Z0-9_-]*\s*(.*?)\s*```").expect("valid code fence regex");
    if let Some(captured) = fenced_block.captures(trimmed) {
        return captured
            .get(1)
            .map(|value| value.as_str().trim().to_string())
            .unwrap_or_default();
    }

    trimmed.to_string()
}

fn markdown_to_plain_text(markdown: &str) -> String {
    markdown
        .lines()
        .map(|line| {
            line.trim_start_matches('#')
                .trim_start_matches('-')
                .trim()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_format(format: &str) -> String {
    match format.trim().to_ascii_lowercase().as_str() {
        "markdown" | "md" => "md".to_string(),
        "txt" | "text" => "txt".to_string(),
        _ => "html".to_string(),
    }
}

fn source_article_length_range(config: &PostpubConfig) -> (usize, usize) {
    let enabled_targets = config
        .publish_targets
        .iter()
        .filter(|target| target.enabled)
        .collect::<Vec<_>>();

    if enabled_targets.is_empty() {
        return (config.min_article_len, config.max_article_len);
    }

    let min_article_len = enabled_targets
        .iter()
        .map(|target| target.min_article_len)
        .max()
        .unwrap_or(config.min_article_len);
    let max_article_len = enabled_targets
        .iter()
        .map(|target| target.max_article_len)
        .min()
        .unwrap_or(config.max_article_len);

    if min_article_len <= max_article_len {
        (min_article_len, max_article_len)
    } else {
        (config.min_article_len, config.max_article_len)
    }
}

#[derive(Debug, Deserialize)]
struct ChatCompletionsResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

impl ChatCompletionsResponse {
    fn first_text(&self) -> Option<String> {
        self.choices
            .first()
            .and_then(|choice| choice.message.text())
    }
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: ChatMessageContent,
}

impl ChatMessage {
    fn text(&self) -> Option<String> {
        match &self.content {
            ChatMessageContent::Text(text) => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            ChatMessageContent::Parts(parts) => {
                let text = parts
                    .iter()
                    .filter_map(|part| {
                        let trimmed = part.text.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ChatMessageContent {
    Text(String),
    Parts(Vec<ChatMessagePart>),
}

#[derive(Debug, Deserialize)]
struct ChatMessagePart {
    #[serde(default)]
    text: String,
}

#[cfg(test)]
mod tests {
    use postpub_types::{
        CustomLlmProvider, GenerateArticleRequest, PostpubConfig, SearchResult, UiConfig,
    };

    use super::{
        build_design_user_prompt, build_template_user_prompt, chat_completions_endpoint,
        format_sources_for_prompt, normalize_llm_html, normalize_llm_markdown,
        resolve_provider_api_key, select_generation_provider, strip_code_fences,
    };

    #[test]
    fn formats_sources_like_tool_output() {
        let formatted = format_sources_for_prompt(
            &GenerateArticleRequest {
                topic: "topic".to_string(),
                reference_urls: vec!["https://example.com/source".to_string()],
                template_category: None,
                template_name: None,
                save_output: false,
            },
            "reference",
            &[SearchResult {
                title: "Example result".to_string(),
                url: "https://example.com".to_string(),
                abstract_text: "This is a useful summary for the generated article.".to_string(),
                published_at: Some("2026-03-28".to_string()),
                content: Some("Longer content for reuse".to_string()),
            }],
        );

        assert!(formatted.contains("reference sources for 'topic'"));
        assert!(formatted.contains("Title: Example result"));
        assert!(formatted.contains("Published at: 2026-03-28"));
        assert!(formatted.contains("Content: Longer content for reuse"));
    }

    #[test]
    fn normalizes_llm_markdown_title_and_code_fences() {
        let markdown =
            normalize_llm_markdown("```markdown\n# Wrong title\n\n正文内容\n```", "topic");

        assert!(markdown.starts_with("# topic"));
        assert!(markdown.contains("正文内容"));
        assert!(!markdown.contains("```"));
    }

    #[test]
    fn strips_code_fences_only_when_present() {
        assert_eq!(strip_code_fences("plain text"), "plain text");
        assert_eq!(strip_code_fences("```md\nhello\n```"), "hello");
        assert_eq!(
            strip_code_fences("before\n```html\n<section>hello</section>\n```\nafter"),
            "<section>hello</section>"
        );
    }

    #[test]
    fn normalizes_llm_html_from_code_fence() {
        let html =
            normalize_llm_html("```html\n<section>hello</section>\n```").expect("normalized html");
        assert_eq!(html, "<section>hello</section>");
    }

    #[test]
    fn builds_template_prompt_with_template_content() {
        let prompt = build_template_user_prompt(
            "wechat",
            "# 标题\n\n正文",
            "公众号标题",
            "<section style=\"color:red\">template</section>",
            900,
            2000,
        );

        assert!(prompt.contains("wechat"));
        assert!(prompt.contains("<section style=\"color:red\">template</section>"));
        assert!(prompt.contains("900-2000"));
    }

    #[test]
    fn builds_design_prompt_for_platform() {
        let prompt = build_design_user_prompt("wechat", "# 标题\n\n正文", "公众号标题");
        assert!(prompt.contains("wechat"));
        assert!(prompt.contains("公众号标题"));
    }

    #[test]
    fn selects_first_supported_enabled_provider() {
        let ui_config = UiConfig {
            custom_llm_providers: vec![
                CustomLlmProvider {
                    enabled: true,
                    model: "gpt-4o-mini".to_string(),
                    api_base: "https://api.openai.com/v1".to_string(),
                    protocol_type: "openai".to_string(),
                    ..CustomLlmProvider::default()
                },
                CustomLlmProvider {
                    enabled: true,
                    model: "other".to_string(),
                    api_base: "https://example.com/v1".to_string(),
                    protocol_type: "anthropic".to_string(),
                    ..CustomLlmProvider::default()
                },
            ],
            ..UiConfig::default()
        };

        let provider = select_generation_provider(&ui_config).expect("provider");
        assert_eq!(provider.model, "gpt-4o-mini");
    }

    #[test]
    fn provider_selection_fails_when_no_supported_provider_exists() {
        let ui_config = UiConfig {
            custom_llm_providers: vec![CustomLlmProvider {
                enabled: true,
                model: "claude".to_string(),
                api_base: "https://example.com".to_string(),
                protocol_type: "anthropic".to_string(),
                ..CustomLlmProvider::default()
            }],
            ..UiConfig::default()
        };

        let error = select_generation_provider(&ui_config).expect_err("missing provider");
        assert!(error.to_string().contains("no enabled LLM provider"));
    }

    #[test]
    fn accepts_legacy_custom_protocol_provider() {
        let ui_config = UiConfig {
            custom_llm_providers: vec![CustomLlmProvider {
                enabled: true,
                model: "gpt-4o-mini".to_string(),
                api_base: "https://api.openai.com/v1".to_string(),
                protocol_type: "custom".to_string(),
                ..CustomLlmProvider::default()
            }],
            ..UiConfig::default()
        };

        let provider = select_generation_provider(&ui_config).expect("provider");
        assert_eq!(provider.protocol_type, "custom");
    }

    #[test]
    fn returns_error_when_api_key_is_missing() {
        let provider = CustomLlmProvider {
            name: "Test".to_string(),
            api_key: String::new(),
            ..CustomLlmProvider::default()
        };

        let error = resolve_provider_api_key(&provider).expect_err("missing api key");
        assert!(error.to_string().contains("fill api_key"));
    }

    #[test]
    fn builds_chat_completion_endpoint() {
        assert_eq!(
            chat_completions_endpoint("https://api.openai.com/v1", "openai").expect("endpoint"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_endpoint(
                "https://api.openai.com/v1/chat/completions",
                "openai_compatible"
            )
            .expect("endpoint"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_endpoint("https://api.openai.com/v1", "custom").expect("endpoint"),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn computes_article_length_range_from_enabled_targets() {
        let config = PostpubConfig::default();
        let (min_len, max_len) = super::source_article_length_range(&config);
        assert!(min_len <= max_len);
    }
}
