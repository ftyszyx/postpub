use std::time::Duration;

use chrono::{DateTime, Utc};
use postpub_types::{AiforgeConfig, SearchResult};
use reqwest::{header, Client, Url};
use rss::Channel;
use scraper::{Html, Selector};
use tracing::warn;

use crate::error::{PostpubError, Result};

#[derive(Debug, Clone)]
pub struct AiforgeEngine {
    client: Client,
    config: AiforgeConfig,
}

impl AiforgeEngine {
    pub fn new(client: Client, config: AiforgeConfig) -> Self {
        Self { client, config }
    }

    pub async fn collect_sources(
        &self,
        topic: &str,
        reference_urls: &[String],
        min_results: usize,
        max_results: usize,
    ) -> Result<(String, Vec<SearchResult>)> {
        let results = if reference_urls.is_empty() {
            match self.search_news(topic, max_results).await {
                Ok(results) => ("search".to_string(), results),
                Err(error) => {
                    warn!(
                        topic = topic,
                        provider = self.active_search_provider(),
                        error = %error,
                        "source retrieval failed, falling back to topic brief mode"
                    );
                    (
                        "fallback".to_string(),
                        self.build_topic_brief_sources(topic, min_results, max_results),
                    )
                }
            }
        } else {
            (
                "reference".to_string(),
                self.extract_reference_urls(reference_urls).await?,
            )
        };

        let filtered = results
            .1
            .into_iter()
            .filter(|item| {
                !item.title.trim().is_empty()
                    && !item.url.trim().is_empty()
                    && (!item.abstract_text.trim().is_empty()
                        || item
                            .content
                            .as_ref()
                            .is_some_and(|value| !value.trim().is_empty()))
            })
            .take(max_results)
            .collect::<Vec<_>>();

        if filtered.len() < min_results {
            return Err(PostpubError::Validation(format!(
                "not enough sources found for '{topic}': expected at least {min_results}, got {}",
                filtered.len()
            )));
        }

        Ok((results.0, filtered))
    }

    pub async fn search_news(&self, topic: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        match self.active_search_provider() {
            "google_news_rss" => self.search_google_news(topic, max_results).await,
            "topic_brief" => Ok(self.build_topic_brief_sources(topic, 1, max_results)),
            provider => Err(PostpubError::Validation(format!(
                "unsupported search provider: {provider}"
            ))),
        }
    }

    async fn search_google_news(&self, topic: &str, max_results: usize) -> Result<Vec<SearchResult>> {
        let mut url = Url::parse("https://news.google.com/rss/search")?;
        let locale = normalize_locale(&self.config.search.locale);
        url.query_pairs_mut()
            .append_pair("q", topic)
            .append_pair("hl", locale.hl)
            .append_pair("gl", locale.gl)
            .append_pair("ceid", locale.ceid);

        let response = self
            .client
            .get(url)
            .header(header::USER_AGENT, self.config.fetcher.user_agent.as_str())
            .timeout(Duration::from_secs(self.config.search.request_timeout_secs))
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;

        let channel = Channel::read_from(&response[..])
            .map_err(|error| PostpubError::External(format!("failed to parse rss: {error}")))?;

        Ok(channel
            .items()
            .iter()
            .take(max_results)
            .map(|item| SearchResult {
                title: item.title().unwrap_or_default().trim().to_string(),
                url: item.link().unwrap_or_default().trim().to_string(),
                abstract_text: strip_html(item.description().unwrap_or_default()),
                published_at: item.pub_date().and_then(parse_pub_date),
                content: None,
            })
            .collect())
    }

    pub async fn extract_reference_urls(&self, urls: &[String]) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        for url in urls {
            let response = self
                .client
                .get(url)
                .header(header::USER_AGENT, self.config.fetcher.user_agent.as_str())
                .timeout(Duration::from_secs(self.config.fetcher.request_timeout_secs))
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?;

            results.push(parse_reference_html(
                url,
                &response,
                self.config.fetcher.max_content_chars,
            ));
        }

        Ok(results)
    }

    fn active_search_provider(&self) -> &str {
        let provider = self.config.search.provider.trim();
        if provider.is_empty() {
            self.config.default_search_provider.trim()
        } else {
            provider
        }
    }

    fn build_topic_brief_sources(
        &self,
        topic: &str,
        min_results: usize,
        max_results: usize,
    ) -> Vec<SearchResult> {
        let count = min_results.max(1).min(max_results.max(1));
        let trimmed_topic = topic.trim();
        let headline = if trimmed_topic.is_empty() {
            "untitled topic"
        } else {
            trimmed_topic
        };
        let sections = [
            (
                "Topic snapshot",
                "Summarize the core idea, define the scope, and explain why the topic matters right now.",
            ),
            (
                "Audience questions",
                "List the main questions a reader would expect this article to answer and turn them into subheadings.",
            ),
            (
                "Practical angle",
                "Describe examples, workflows, or trade-offs that make the topic concrete and useful.",
            ),
            (
                "Risks and caveats",
                "Call out uncertainty, missing evidence, and places where manual verification is still needed.",
            ),
            (
                "Next steps",
                "Suggest follow-up research directions, comparisons, and examples to strengthen the next draft.",
            ),
        ];

        (0..count)
            .map(|index| {
                let section = sections.get(index).copied().unwrap_or((
                    "Additional perspective",
                    "Add another angle that expands the article without repeating earlier sections.",
                ));

                SearchResult {
                    title: format!("{}: {}", headline, section.0),
                    url: format!("about:blank#postpub-fallback-{}", index + 1),
                    abstract_text: format!(
                        "External news retrieval is unavailable, so this draft uses an internal topic brief. {}",
                        section.1
                    ),
                    published_at: Some(Utc::now().format("%Y-%m-%d").to_string()),
                    content: Some(format!(
                        "Topic: {headline}\nSection: {}\nGuidance: {}\nNote: Replace this fallback brief with verified external sources when network access becomes available.",
                        section.0, section.1
                    )),
                }
            })
            .collect()
    }
}

struct SearchLocale<'a> {
    hl: &'a str,
    gl: &'a str,
    ceid: &'a str,
}

fn normalize_locale(locale: &str) -> SearchLocale<'static> {
    match locale.trim().to_ascii_lowercase().as_str() {
        "en-us" | "en_us" => SearchLocale {
            hl: "en-US",
            gl: "US",
            ceid: "US:en",
        },
        _ => SearchLocale {
            hl: "zh-CN",
            gl: "CN",
            ceid: "CN:zh-Hans",
        },
    }
}

fn parse_pub_date(value: &str) -> Option<String> {
    DateTime::parse_from_rfc2822(value)
        .map(|date| date.with_timezone(&Utc).format("%Y-%m-%d").to_string())
        .ok()
        .or_else(|| {
            DateTime::parse_from_rfc3339(value)
                .map(|date| date.with_timezone(&Utc).format("%Y-%m-%d").to_string())
                .ok()
        })
}

pub fn parse_reference_html(url: &str, html: &str, max_content_chars: usize) -> SearchResult {
    let document = Html::parse_document(html);

    let title = extract_first_text(
        &document,
        &[
            "meta[property='og:title']",
            "meta[name='title']",
            "title",
            "h1",
            ".article-title",
            ".post-title",
        ],
        true,
    );
    let published_at = extract_published_at(&document);
    let content = extract_content(&document, max_content_chars);

    SearchResult {
        title,
        url: url.to_string(),
        abstract_text: summarize_text(&content, 280),
        published_at,
        content: Some(content),
    }
}

fn extract_first_text(document: &Html, selectors: &[&str], content_attr: bool) -> String {
    for raw_selector in selectors {
        let Ok(selector) = Selector::parse(raw_selector) else {
            continue;
        };
        if let Some(element) = document.select(&selector).next() {
            if content_attr {
                if let Some(content) = element.value().attr("content") {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return trimmed.to_string();
                    }
                }
            }

            let text = element
                .text()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !text.is_empty() {
                return text;
            }
        }
    }

    String::new()
}

fn extract_published_at(document: &Html) -> Option<String> {
    for selector in [
        "meta[property='article:published_time']",
        "meta[itemprop='datePublished']",
        "meta[name='publishdate']",
        "time[datetime]",
    ] {
        let Ok(parsed) = Selector::parse(selector) else {
            continue;
        };
        if let Some(element) = document.select(&parsed).next() {
            if let Some(value) = element
                .value()
                .attr("content")
                .or_else(|| element.value().attr("datetime"))
            {
                let trimmed = value.trim();
                if trimmed.len() >= 10 {
                    return Some(trimmed.chars().take(10).collect());
                }
            }
        }
    }

    None
}

fn extract_content(document: &Html, max_content_chars: usize) -> String {
    for selector in [
        "#js_content",
        ".rich_media_content",
        "article",
        "main",
        ".article-content",
        ".post-content",
        ".entry-content",
        ".content",
        "body",
    ] {
        let Ok(parsed) = Selector::parse(selector) else {
            continue;
        };
        if let Some(container) = document.select(&parsed).next() {
            let text = container
                .text()
                .map(str::trim)
                .filter(|value| value.len() > 1)
                .collect::<Vec<_>>()
                .join("\n");
            let cleaned = normalize_whitespace(&text);
            if cleaned.len() > 40 {
                return truncate_chars(&cleaned, max_content_chars);
            }
        }
    }

    String::new()
}

fn summarize_text(text: &str, max_chars: usize) -> String {
    truncate_chars(text, max_chars)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut result = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= max_chars {
            result.push_str("...");
            break;
        }
        result.push(ch);
    }
    result
}

fn strip_html(html: &str) -> String {
    let mut output = String::with_capacity(html.len());
    let mut in_tag = false;

    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }

    normalize_whitespace(&output)
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use postpub_types::{AiforgeConfig, FetcherConfig, SearchProviderConfig};
    use reqwest::Client;

    use super::{normalize_locale, parse_reference_html, strip_html, AiforgeEngine};

    #[test]
    fn strips_html_snippets() {
        assert_eq!(
            strip_html("<p>Hello <strong>world</strong></p>"),
            "Hello world"
        );
    }

    #[test]
    fn extracts_reference_content_from_html() {
        let html = r#"
        <html>
          <head>
            <title>Example Story</title>
            <meta property="article:published_time" content="2026-03-28T08:00:00Z" />
          </head>
          <body>
            <article>
              <p>Paragraph one with enough content to be useful.</p>
              <p>Paragraph two adds extra details for the draft.</p>
            </article>
          </body>
        </html>
        "#;
        let result = parse_reference_html("https://example.com/story", html, 1000);

        assert_eq!(result.title, "Example Story");
        assert_eq!(result.published_at.as_deref(), Some("2026-03-28"));
        assert!(result
            .content
            .as_deref()
            .unwrap_or_default()
            .contains("Paragraph one"));
    }

    #[test]
    fn falls_back_to_topic_brief_sources() {
        let engine = AiforgeEngine::new(
            Client::new(),
            AiforgeConfig {
                default_search_provider: "topic_brief".to_string(),
                search: SearchProviderConfig {
                    provider: "topic_brief".to_string(),
                    max_results: 8,
                    request_timeout_secs: 15,
                    locale: "zh-CN".to_string(),
                },
                fetcher: FetcherConfig::default(),
            },
        );

        let results = engine.build_topic_brief_sources("vicoding", 3, 8);

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|item| item.title.contains("vicoding")));
        assert!(results
            .iter()
            .all(|item| item.abstract_text.contains("External news retrieval is unavailable")));
    }

    #[test]
    fn normalizes_google_news_locale() {
        let english = normalize_locale("en-US");
        assert_eq!(english.hl, "en-US");
        assert_eq!(english.gl, "US");
        assert_eq!(english.ceid, "US:en");

        let chinese = normalize_locale("zh-CN");
        assert_eq!(chinese.hl, "zh-CN");
        assert_eq!(chinese.gl, "CN");
        assert_eq!(chinese.ceid, "CN:zh-Hans");
    }
}
