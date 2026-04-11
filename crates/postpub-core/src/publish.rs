use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use postpub_types::{
    ArticleDesign, ArticleDocument, ArticleVariantDocument, PublishArticleRequest, PublishOutput,
    PublishTargetConfig,
};
use regex::Regex;
use tokio::{
    io::AsyncWriteExt,
    process::Command,
    time::{sleep, Instant},
};

use crate::browser::{browser_profile_dir, sanitize_profile_component};
use crate::{preview_html, AppContext, PostpubError, Result};

#[derive(Clone)]
pub struct PublishProgressReporter {
    callback: Arc<dyn Fn(String, String) + Send + Sync>,
}

impl PublishProgressReporter {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(String, String) + Send + Sync + 'static,
    {
        Self {
            callback: Arc::new(callback),
        }
    }

    pub fn report(&self, stage: impl Into<String>, message: impl Into<String>) {
        (self.callback)(stage.into(), message.into());
    }
}

#[async_trait]
pub trait BrowserRuntime: Send + Sync {
    async fn open(&self, _url: &str) -> Result<()> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn click(&self, _selector: &str) -> Result<()> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn fill(&self, _selector: &str, _text: &str) -> Result<()> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn upload(&self, _selector: &str, _files: &[String]) -> Result<()> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn get_url(&self) -> Result<String> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn get_text(&self, _selector: &str) -> Result<String> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn evaluate(&self, _script: &str) -> Result<String> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn press(&self, _key: &str) -> Result<()> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn keyboard_insert_text(&self, _text: &str) -> Result<()> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn wait_load(&self, _state: &str) -> Result<()> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn wait_ms(&self, _ms: u64) -> Result<()> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }

    async fn screenshot(&self, _path: &Path) -> Result<()> {
        Err(PostpubError::External(
            "browser runtime is not configured yet".to_string(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct AgentBrowserRuntime {
    program: OsString,
    session_name: String,
    headed: bool,
    browser_executable: Option<PathBuf>,
    browser_profile: Option<PathBuf>,
}

impl AgentBrowserRuntime {
    pub fn new(
        session_name: impl Into<String>,
        browser_executable: Option<PathBuf>,
        browser_profile: Option<PathBuf>,
    ) -> Self {
        Self {
            program: resolve_agent_browser_program(),
            session_name: session_name.into(),
            headed: env_flag("POSTPUB_AGENT_BROWSER_HEADED"),
            browser_executable,
            browser_profile,
        }
    }

    async fn run(&self, args: &[&str]) -> Result<String> {
        self.run_with_stdin(args, None).await
    }

    async fn run_with_stdin(&self, args: &[&str], stdin: Option<&str>) -> Result<String> {
        let mut command = build_agent_browser_command(&self.program);
        if self.headed {
            command.arg("--headed");
        }
        if let Some(path) = &self.browser_executable {
            command.arg("--executable-path").arg(path);
        }
        if let Some(path) = &self.browser_profile {
            command.arg("--profile").arg(path);
        }
        command.arg("--session-name").arg(&self.session_name);
        command.args(args);
        if stdin.is_some() {
            command.stdin(std::process::Stdio::piped());
        }
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        let mut child = command.spawn()?;
        if let Some(payload) = stdin {
            let Some(mut handle) = child.stdin.take() else {
                return Err(PostpubError::External(
                    "agent-browser stdin is unavailable".to_string(),
                ));
            };
            handle.write_all(payload.as_bytes()).await?;
            handle.shutdown().await?;
        }

        let output = child.wait_with_output().await?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if output.status.success() {
            Ok(stdout)
        } else {
            let detail = if !stderr.is_empty() { stderr } else { stdout };
            Err(PostpubError::External(format!(
                "agent-browser {} failed: {}",
                args.join(" "),
                detail
            )))
        }
    }
}

fn resolve_agent_browser_program() -> OsString {
    if let Some(path) = env::var_os("POSTPUB_AGENT_BROWSER_BIN") {
        return path;
    }

    #[cfg(windows)]
    let candidates = ["agent-browser.cmd", "agent-browser.exe", "agent-browser.bat"];
    #[cfg(not(windows))]
    let candidates = ["agent-browser"];

    if let Some(path) = find_program_in_path(&candidates) {
        return path.into_os_string();
    }

    #[cfg(windows)]
    {
        OsString::from("agent-browser.cmd")
    }
    #[cfg(not(windows))]
    {
        OsString::from("agent-browser")
    }
}

fn find_program_in_path(candidates: &[&str]) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;

    for directory in env::split_paths(&path) {
        for candidate in candidates {
            let full_path = directory.join(candidate);
            if full_path.is_file() {
                return Some(full_path);
            }
        }
    }

    None
}

fn build_agent_browser_command(program: &OsString) -> Command {
    let extension = Path::new(program)
        .extension()
        .and_then(|item| item.to_str())
        .map(|item| item.to_ascii_lowercase());

    match extension.as_deref() {
        Some("cmd") | Some("bat") => {
            let mut command = Command::new("cmd");
            command.arg("/C").arg(program);
            command
        }
        Some("ps1") => {
            let mut command = Command::new("powershell");
            command
                .arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(program);
            command
        }
        _ => Command::new(program),
    }
}

#[async_trait]
impl BrowserRuntime for AgentBrowserRuntime {
    async fn open(&self, url: &str) -> Result<()> {
        self.run(&["open", url]).await.map(|_| ())
    }

    async fn click(&self, selector: &str) -> Result<()> {
        self.run(&["click", selector]).await.map(|_| ())
    }

    async fn fill(&self, selector: &str, text: &str) -> Result<()> {
        self.run(&["fill", selector, text]).await.map(|_| ())
    }

    async fn upload(&self, selector: &str, files: &[String]) -> Result<()> {
        let mut args = vec!["upload", selector];
        let owned_files: Vec<&str> = files.iter().map(|item| item.as_str()).collect();
        args.extend(owned_files);
        self.run(&args).await.map(|_| ())
    }

    async fn get_url(&self) -> Result<String> {
        self.run(&["get", "url"]).await
    }

    async fn get_text(&self, selector: &str) -> Result<String> {
        self.run(&["get", "text", selector]).await
    }

    async fn evaluate(&self, script: &str) -> Result<String> {
        let encoded = BASE64_STANDARD.encode(script.as_bytes());
        self.run(&["eval", "-b", &encoded]).await
    }

    async fn press(&self, key: &str) -> Result<()> {
        self.run(&["press", key]).await.map(|_| ())
    }

    async fn keyboard_insert_text(&self, text: &str) -> Result<()> {
        self.run(&["keyboard", "inserttext", text])
            .await
            .map(|_| ())
    }

    async fn wait_load(&self, state: &str) -> Result<()> {
        self.run(&["wait", "--load", state]).await.map(|_| ())
    }

    async fn wait_ms(&self, ms: u64) -> Result<()> {
        self.run(&["wait", &ms.to_string()]).await.map(|_| ())
    }

    async fn screenshot(&self, path: &Path) -> Result<()> {
        let path_string = path.to_string_lossy().to_string();
        self.run(&["screenshot", &path_string]).await.map(|_| ())
    }
}

#[async_trait]
pub trait Publisher: Send + Sync {
    fn platform_type(&self) -> &'static str;

    async fn publish(
        &self,
        target: &PublishTargetConfig,
        article: &ArticleDocument,
        variant: &ArticleVariantDocument,
        request: &PublishArticleRequest,
        reporter: &PublishProgressReporter,
    ) -> Result<PublishOutput>;
}

pub struct PublishService {
    context: AppContext,
}

impl PublishService {
    pub fn new(context: AppContext) -> Self {
        Self { context }
    }

    pub async fn publish_with_progress(
        &self,
        request: PublishArticleRequest,
        reporter: PublishProgressReporter,
    ) -> Result<PublishOutput> {
        let mode = request.mode.trim().to_lowercase();
        if mode != "draft" && mode != "publish" {
            return Err(PostpubError::Validation(format!(
                "unsupported publish mode: {}",
                request.mode
            )));
        }

        reporter.report(
            "prepare",
            format!("加载文章 {}", request.article_relative_path),
        );
        let article = self
            .context
            .article_store()
            .get_article(&request.article_relative_path)?;

        reporter.report("target", format!("解析发布目标 {}", request.target_id));
        let bundle = self.context.config_store().load_bundle()?;
        let target = bundle
            .config
            .publish_targets
            .into_iter()
            .find(|item| item.id == request.target_id)
            .ok_or_else(|| {
                PostpubError::NotFound(format!("publish target not found: {}", request.target_id))
            })?;

        if !target.enabled {
            return Err(PostpubError::Conflict(format!(
                "publish target is disabled: {}",
                target.id
            )));
        }

        reporter.report("variant", format!("定位文章变体 {}", request.target_id));
        let variant = article
            .variants
            .iter()
            .find(|item| item.summary.target_id == request.target_id)
            .ok_or_else(|| {
                PostpubError::NotFound(format!(
                    "article variant not found for target: {}",
                    request.target_id
                ))
            })?;

        reporter.report(
            "platform",
            format!("调用平台适配器 {}", target.platform_type),
        );
        match target.platform_type.as_str() {
            "wechat" => {
                let publisher = WechatPublisher::new(self.context.clone());
                publisher
                    .publish(&target, &article, variant, &request, &reporter)
                    .await
            }
            other => Err(PostpubError::Validation(format!(
                "publish platform is not supported yet: {other}"
            ))),
        }
    }
}

#[derive(Clone)]
pub struct WechatPublisher {
    context: AppContext,
}

impl WechatPublisher {
    pub fn new(context: AppContext) -> Self {
        Self { context }
    }

    async fn publish_with_runtime<R: BrowserRuntime>(
        &self,
        runtime: &R,
        target: &PublishTargetConfig,
        article: &ArticleDocument,
        variant: &ArticleVariantDocument,
        request: &PublishArticleRequest,
        reporter: &PublishProgressReporter,
    ) -> Result<PublishOutput> {
        if request.mode.trim().eq_ignore_ascii_case("publish") {
            return Err(PostpubError::Validation(
                "wechat publish mode is not implemented yet; use draft mode first".to_string(),
            ));
        }

        reporter.report(
            "wechat.prepare",
            format!(
                "准备发布到微信公众号：{}",
                if target.name.trim().is_empty() {
                    target.id.as_str()
                } else {
                    target.name.as_str()
                }
            ),
        );

        let title = article_title(article, variant);
        let cover_path = resolve_wechat_cover_path(&self.context, target, article)?;
        let body_html = article_body_html(article, variant);
        let origin = wechat_origin(&target.publish_url)?;
        validate_wechat_supported_settings(target)?;

        reporter.report("wechat.browser", "启动 agent-browser 会话");
        runtime.open(&target.publish_url).await?;
        wait_until_true(
            runtime,
            r#"(() => document.readyState === "complete")()"#,
            Duration::from_secs(15),
        )
        .await?;

        reporter.report("wechat.auth", "检查微信公众号登录状态");
        let current_url = runtime.get_url().await?;
        let Some(token) = extract_query_value(&current_url, "token") else {
            let body_text = runtime.get_text("body").await.unwrap_or_default();
            if body_text.contains("使用账号登录") || body_text.contains("登录") {
                return Err(PostpubError::External(format!(
                    "wechat login is required. Please log in once with agent-browser session '{}'",
                    resolve_wechat_session_name(target)
                )));
            }
            return Err(PostpubError::External(format!(
                "failed to determine wechat token from current url: {current_url}"
            )));
        };

        let draft_url = format!(
            "{origin}/cgi-bin/appmsg?begin=0&count=10&type=77&action=list_card&token={token}&lang=zh_CN"
        );
        reporter.report("wechat.navigate", "进入草稿箱");
        runtime.open(&draft_url).await?;
        wait_for_text(runtime, "新的创作", Duration::from_secs(15)).await?;

        reporter.report("wechat.editor", "打开新文章编辑页");
        click_by_text(runtime, "新的创作", true).await?;
        wait_for_text(runtime, "写新文章", Duration::from_secs(10)).await?;
        click_by_text(runtime, "写新文章", true).await?;
        wait_until_true(
            runtime,
            r#"(() => !!document.querySelector(".js_title") && !!document.querySelector(".ProseMirror"))()"#,
            Duration::from_secs(15),
        )
        .await?;

        reporter.report("wechat.title", "填写文章标题");
        runtime.fill(".js_title", &title).await?;

        if !target.account_name.trim().is_empty() {
            reporter.report("wechat.author", "填写作者信息");
            runtime
                .fill(".js_author", target.account_name.trim())
                .await?;
        }

        reporter.report("wechat.body", "写入文章正文");
        set_editor_html(runtime, &body_html).await?;
        wait_until_true(
            runtime,
            r#"(() => {
                const editor = document.querySelector(".ProseMirror");
                return !!editor && (editor.innerText || "").trim().length > 0;
            })()"#,
            Duration::from_secs(10),
        )
        .await?;

        reporter.report("wechat.cover", "打开封面选择面板");
        runtime.click("#js_cover_area").await?;
        wait_until_true(
            runtime,
            r##"(() => !!document.querySelector("#js_cover_area .js_imagedialog"))()"##,
            Duration::from_secs(10),
        )
        .await?;

        reporter.report("wechat.cover.upload", "进入图片库并上传封面");
        runtime.click("#js_cover_area .js_imagedialog").await?;
        wait_for_text(runtime, "选择图片", Duration::from_secs(10)).await?;
        runtime
            .upload(
                r#"div[id^="rt_"] input[type="file"]"#,
                &[cover_path.to_string_lossy().to_string()],
            )
            .await?;

        let file_name = cover_path
            .file_name()
            .map(|item| item.to_string_lossy().to_string())
            .ok_or_else(|| PostpubError::InvalidPath(cover_path.display().to_string()))?;
        wait_for_text(runtime, &file_name, Duration::from_secs(20)).await?;

        reporter.report("wechat.cover.crop", "确认封面裁剪");
        click_by_text(runtime, "下一步", true).await?;
        wait_for_text(runtime, "编辑封面", Duration::from_secs(10)).await?;
        click_by_text(runtime, "确认", true).await?;
        wait_until_true(
            runtime,
            r##"(() => {
                const preview = document.querySelector("#js_cover_area .js_cover_preview_new");
                return !!preview && getComputedStyle(preview).backgroundImage && getComputedStyle(preview).backgroundImage !== "none";
            })()"##,
            Duration::from_secs(15),
        )
        .await?;

        apply_wechat_publish_settings(runtime, target, reporter).await?;

        reporter.report("wechat.save", "保存为草稿");
        click_by_text(runtime, "保存为草稿", true).await?;
        wait_for_text(runtime, "手动保存", Duration::from_secs(20)).await?;

        let remote_url = runtime.get_url().await.ok();
        let remote_id = remote_url
            .as_deref()
            .and_then(|url| extract_query_value(url, "appmsgid"));

        reporter.report("wechat.done", "微信公众号草稿保存完成");
        Ok(PublishOutput {
            article_relative_path: article.summary.relative_path.clone(),
            article_title: title,
            target_id: target.id.clone(),
            target_name: target.name.clone(),
            platform_type: target.platform_type.clone(),
            mode: request.mode.clone(),
            format: variant.summary.format.clone(),
            remote_id,
            remote_url,
        })
    }
}

#[async_trait]
impl Publisher for WechatPublisher {
    fn platform_type(&self) -> &'static str {
        "wechat"
    }

    async fn publish(
        &self,
        target: &PublishTargetConfig,
        article: &ArticleDocument,
        variant: &ArticleVariantDocument,
        request: &PublishArticleRequest,
        reporter: &PublishProgressReporter,
    ) -> Result<PublishOutput> {
        reporter.report("wechat.browser.prepare", "检查内置浏览器版本");
        let browser_executable = self
            .context
            .browser_manager()
            .ensure_browser_executable_with_progress(|stage, message| {
                reporter.report(format!("wechat.{stage}"), message.to_string())
            })
            .await?;
        reporter.report(
            "wechat.browser.prepare",
            format!("使用浏览器 {}", browser_executable.display()),
        );
        let browser_profile = resolve_browser_profile_dir(&self.context, target)?;
        fs::create_dir_all(&browser_profile)?;
        reporter.report(
            "wechat.browser.profile",
            format!("使用独立浏览器环境 {}", browser_profile.display()),
        );
        let runtime = AgentBrowserRuntime::new(
            resolve_wechat_session_name(target),
            Some(browser_executable),
            Some(browser_profile),
        );
        self.publish_with_runtime(&runtime, target, article, variant, request, reporter)
            .await
    }
}

fn article_title(article: &ArticleDocument, variant: &ArticleVariantDocument) -> String {
    extract_first_heading(&variant.preview_html)
        .or_else(|| extract_first_heading(&variant.content))
        .unwrap_or_else(|| article.summary.title.clone())
}

fn article_body_html(article: &ArticleDocument, variant: &ArticleVariantDocument) -> String {
    let format = variant.summary.format.as_str();
    let preview = if variant.preview_html.trim().is_empty() {
        preview_html(format, &variant.content)
    } else {
        variant.preview_html.clone()
    };

    if preview.trim().is_empty() {
        preview_html(article.summary.format.as_str(), &article.content)
    } else {
        preview
    }
}

fn resolve_wechat_session_name(target: &PublishTargetConfig) -> String {
    env::var("POSTPUB_AGENT_BROWSER_SESSION_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            let suffix = sanitize_profile_component(if target.id.trim().is_empty() {
                "default"
            } else {
                target.id.trim()
            });
            format!("postpub-{}-{suffix}", target.platform_type.trim())
        })
}

fn resolve_browser_profile_dir(
    context: &AppContext,
    target: &PublishTargetConfig,
) -> Result<PathBuf> {
    if let Ok(value) = env::var("POSTPUB_BROWSER_PROFILE_ROOT") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed).join(sanitize_profile_component(
                if target.id.trim().is_empty() {
                    "default"
                } else {
                    target.id.trim()
                },
            )));
        }
    }

    Ok(browser_profile_dir(
        context.paths(),
        if target.id.trim().is_empty() {
            "default"
        } else {
            target.id.trim()
        },
    ))
}

fn wechat_origin(publish_url: &str) -> Result<String> {
    let parsed =
        url::Url::parse(publish_url).or_else(|_| url::Url::parse("https://mp.weixin.qq.com"))?;
    Ok(format!(
        "{}://{}",
        parsed.scheme(),
        parsed.host_str().unwrap_or("mp.weixin.qq.com")
    ))
}

fn extract_query_value(url: &str, key: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    parsed
        .query_pairs()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.to_string())
}

fn resolve_wechat_cover_path(
    context: &AppContext,
    target: &PublishTargetConfig,
    article: &ArticleDocument,
) -> Result<PathBuf> {
    let design = context
        .article_store()
        .load_article_design(&article.summary.relative_path)
        .unwrap_or_else(|_| ArticleDesign::default());

    let raw_path = match target.wechat.cover_strategy.trim() {
        "custom_path" => target.wechat.cover_path.trim().to_string(),
        "article_cover" => {
            if !design.cover.trim().is_empty() {
                design.cover.trim().to_string()
            } else if !target.wechat.cover_path.trim().is_empty() {
                target.wechat.cover_path.trim().to_string()
            } else {
                String::new()
            }
        }
        "manual" => {
            return Err(PostpubError::Validation(
                "wechat cover strategy 'manual' is not supported in automated publish yet"
                    .to_string(),
            ));
        }
        "first_image" => {
            return Err(PostpubError::Validation(
                "wechat cover strategy 'first_image' is not automated yet; use a local cover image first"
                    .to_string(),
            ));
        }
        other => {
            return Err(PostpubError::Validation(format!(
                "unsupported wechat cover strategy: {other}"
            )));
        }
    };

    if raw_path.trim().is_empty() {
        return Err(PostpubError::Validation(
            "wechat cover image is not configured. Set a local cover path or save an article cover first"
                .to_string(),
        ));
    }

    resolve_local_cover_path(context, article, &raw_path)
        .ok_or_else(|| PostpubError::NotFound(format!("wechat cover file not found: {raw_path}")))
}

fn resolve_local_cover_path(
    context: &AppContext,
    article: &ArticleDocument,
    raw_path: &str,
) -> Option<PathBuf> {
    let candidate = PathBuf::from(raw_path);
    let article_path = context
        .paths()
        .articles_dir()
        .join(article.summary.relative_path.replace('/', "\\"));

    let mut checks = Vec::new();
    if candidate.is_absolute() {
        checks.push(candidate.clone());
    } else {
        checks.push(candidate.clone());
    }

    let trimmed = raw_path
        .trim_start_matches('/')
        .trim_start_matches('\\')
        .to_string();
    if !trimmed.is_empty() {
        checks.push(context.paths().app_root().join(&trimmed));
        checks.push(context.paths().images_dir().join(&trimmed));
        if let Some(parent) = article_path.parent() {
            checks.push(parent.join(&trimmed));
        }
    }

    checks
        .into_iter()
        .find(|path| path.exists() && path.is_file())
        .and_then(|path| path.canonicalize().ok().or(Some(path)))
}

fn validate_wechat_supported_settings(target: &PublishTargetConfig) -> Result<()> {
    if target.wechat.enable_reward {
        return Err(PostpubError::Validation(
            "wechat reward automation is not implemented yet".to_string(),
        ));
    }

    if target.wechat.enable_paid {
        return Err(PostpubError::Validation(
            "wechat paid automation is not implemented yet".to_string(),
        ));
    }

    if !target.wechat.collection_id.trim().is_empty() {
        return Err(PostpubError::Validation(
            "wechat collection selection is not implemented yet".to_string(),
        ));
    }

    Ok(())
}

async fn apply_wechat_publish_settings<R: BrowserRuntime>(
    runtime: &R,
    target: &PublishTargetConfig,
    reporter: &PublishProgressReporter,
) -> Result<()> {
    reporter.report("wechat.settings", "应用微信公众号平台设置");

    if target.wechat.declare_original {
        reporter.report("wechat.settings.original", "处理原创声明");
        open_wechat_setting(runtime, "原创").await?;
        runtime.wait_ms(500).await?;
        click_any_text(runtime, &["声明原创", "原创"], true).await?;
        click_optional_text(runtime, &["确定", "确认", "保存"], true).await?;
        runtime.wait_ms(500).await?;
    }

    if target.wechat.comment_mode.trim() != "auto_selected_open" {
        reporter.report("wechat.settings.comment", "更新留言设置");
        open_wechat_setting(runtime, "留言").await?;
        runtime.wait_ms(500).await?;
        let option_text = match target.wechat.comment_mode.trim() {
            "open_all" => "全部开放",
            "closed" => "关闭",
            other => {
                return Err(PostpubError::Validation(format!(
                    "unsupported wechat comment mode: {other}"
                )))
            }
        };
        click_any_text(runtime, &[option_text], true).await?;
        click_optional_text(runtime, &["确定", "确认", "保存"], true).await?;
        runtime.wait_ms(500).await?;
    }

    if !target.wechat.source_url.trim().is_empty() {
        reporter.report("wechat.settings.source_url", "填写原文链接");
        open_wechat_setting(runtime, "原文链接").await?;
        runtime.wait_ms(500).await?;
        fill_first_visible_text_input(runtime, Some("原文"), target.wechat.source_url.trim())
            .await?;
        click_any_text(runtime, &["确定", "确认", "保存"], true).await?;
        runtime.wait_ms(500).await?;
    }

    if !target.wechat.source_label.trim().is_empty() {
        reporter.report("wechat.settings.source_label", "填写创作来源");
        open_wechat_setting(runtime, "创作来源").await?;
        runtime.wait_ms(500).await?;
        fill_first_visible_text_input(runtime, Some("来源"), target.wechat.source_label.trim())
            .await?;
        click_any_text(runtime, &["确定", "确认", "保存"], true).await?;
        runtime.wait_ms(500).await?;
    }

    if !target.wechat.platform_recommendation_enabled {
        reporter.report("wechat.settings.recommendation", "关闭平台推荐");
        set_wechat_row_switch(runtime, "平台推荐", false).await?;
        runtime.wait_ms(500).await?;
    }

    Ok(())
}

async fn click_by_text<R: BrowserRuntime>(runtime: &R, text: &str, exact: bool) -> Result<()> {
    let matcher = serde_json::to_string(text)?;
    let script = format!(
        r#"
(() => {{
  const expected = {matcher};
  const exact = {exact};
  const nodes = Array.from(document.querySelectorAll("a,button,div,li,span"));
  const candidate = nodes
    .filter((node) => {{
      const rect = node.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) {{
        return false;
      }}
      const value = (node.innerText || node.textContent || "").replace(/\s+/g, " ").trim();
      return exact ? value === expected : value.includes(expected);
    }})
    .sort((left, right) => {{
      const leftScore = (left.innerText || left.textContent || "").replace(/\s+/g, " ").trim().length;
      const rightScore = (right.innerText || right.textContent || "").replace(/\s+/g, " ").trim().length;
      return leftScore - rightScore;
    }})[0];
  if (!candidate) {{
    return false;
  }}
  candidate.click();
  return true;
}})()
"#
    );

    if evaluate_bool(runtime, &script).await? {
        Ok(())
    } else {
        Err(PostpubError::External(format!(
            "failed to click wechat element by text: {text}"
        )))
    }
}

async fn click_any_text<R: BrowserRuntime>(
    runtime: &R,
    candidates: &[&str],
    exact: bool,
) -> Result<()> {
    for candidate in candidates {
        if click_by_text(runtime, candidate, exact).await.is_ok() {
            return Ok(());
        }
    }

    Err(PostpubError::External(format!(
        "failed to click any wechat text target: {}",
        candidates.join(", ")
    )))
}

async fn click_optional_text<R: BrowserRuntime>(
    runtime: &R,
    candidates: &[&str],
    exact: bool,
) -> Result<bool> {
    for candidate in candidates {
        if click_by_text(runtime, candidate, exact).await.is_ok() {
            return Ok(true);
        }
    }

    Ok(false)
}

async fn open_wechat_setting<R: BrowserRuntime>(runtime: &R, label: &str) -> Result<()> {
    click_by_text(runtime, label, false).await
}

async fn fill_first_visible_text_input<R: BrowserRuntime>(
    runtime: &R,
    hint: Option<&str>,
    value: &str,
) -> Result<()> {
    let value_json = serde_json::to_string(value)?;
    let hint_json = serde_json::to_string(&hint.unwrap_or_default())?;
    let script = format!(
        r#"
(() => {{
  const normalize = (input) => (input || "").replace(/\s+/g, " ").trim();
  const hint = normalize({hint_json});
  const fields = Array.from(document.querySelectorAll("input, textarea"))
    .filter((field) => {{
      const rect = field.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) {{
        return false;
      }}
      if (field.disabled || field.readOnly) {{
        return false;
      }}
      const type = (field.getAttribute("type") || "text").toLowerCase();
      return type !== "hidden" && type !== "file" && type !== "checkbox" && type !== "radio";
    }});

  const match = fields.find((field) => {{
    if (!hint) {{
      return true;
    }}
    const placeholder = normalize(field.getAttribute("placeholder"));
    const ariaLabel = normalize(field.getAttribute("aria-label"));
    return placeholder.includes(hint) || ariaLabel.includes(hint);
  }}) || fields[0];

  if (!match) {{
    return false;
  }}

  const nativeInputValueSetter =
    Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, "value")?.set;
  const nativeTextAreaValueSetter =
    Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype, "value")?.set;

  if (match instanceof HTMLTextAreaElement && nativeTextAreaValueSetter) {{
    nativeTextAreaValueSetter.call(match, {value_json});
  }} else if (match instanceof HTMLInputElement && nativeInputValueSetter) {{
    nativeInputValueSetter.call(match, {value_json});
  }} else {{
    match.value = {value_json};
  }}

  match.dispatchEvent(new Event("input", {{ bubbles: true }}));
  match.dispatchEvent(new Event("change", {{ bubbles: true }}));
  return true;
}})()
"#
    );

    if evaluate_bool(runtime, &script).await? {
        Ok(())
    } else {
        Err(PostpubError::External(
            "failed to fill visible wechat text input".to_string(),
        ))
    }
}

async fn set_wechat_row_switch<R: BrowserRuntime>(
    runtime: &R,
    label: &str,
    enabled: bool,
) -> Result<()> {
    let label_json = serde_json::to_string(label)?;
    let script = format!(
        r#"
(() => {{
  const normalize = (input) => (input || "").replace(/\s+/g, " ").trim();
  const expected = {label_json};
  const wantEnabled = {enabled};
  const nodes = Array.from(document.querySelectorAll("div,li,section,label"));
  const row = nodes
    .filter((node) => {{
      const rect = node.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) {{
        return false;
      }}
      const text = normalize(node.innerText || node.textContent);
      return text.includes(expected);
    }})
    .sort((left, right) => normalize(left.innerText || left.textContent).length - normalize(right.innerText || right.textContent).length)[0];

  if (!row) {{
    return false;
  }}

  const text = normalize(row.innerText || row.textContent);
  if (wantEnabled && (text.includes("已开启") || text.includes("开启"))) {{
    return true;
  }}
  if (!wantEnabled && (text.includes("已关闭") || text.includes("关闭"))) {{
    return true;
  }}

  const switchNode = row.querySelector('[role="switch"], input[type="checkbox"], button[aria-checked]');
  if (switchNode) {{
    switchNode.click();
    return true;
  }}

  row.click();
  return true;
}})()
"#
    );

    if evaluate_bool(runtime, &script).await? {
        Ok(())
    } else {
        Err(PostpubError::External(format!(
            "failed to set wechat switch for row: {label}"
        )))
    }
}

async fn set_editor_html<R: BrowserRuntime>(runtime: &R, html: &str) -> Result<()> {
    let html_json = serde_json::to_string(html)?;
    let script = format!(
        r#"
(() => {{
  const editor = document.querySelector(".ProseMirror");
  if (!editor) {{
    return false;
  }}
  editor.focus();
  editor.innerHTML = {html_json};
  editor.dispatchEvent(new Event("input", {{ bubbles: true }}));
  editor.dispatchEvent(new Event("change", {{ bubbles: true }}));
  return (editor.innerText || "").trim().length > 0;
}})()
"#
    );

    if evaluate_bool(runtime, &script).await? {
        Ok(())
    } else {
        Err(PostpubError::External(
            "failed to write content into wechat editor".to_string(),
        ))
    }
}

async fn wait_for_text<R: BrowserRuntime>(
    runtime: &R,
    expected: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let text = runtime.get_text("body").await.unwrap_or_default();
        if text.contains(expected) {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(PostpubError::External(format!(
                "timed out waiting for wechat text: {expected}"
            )));
        }

        sleep(Duration::from_millis(500)).await;
    }
}

async fn wait_until_true<R: BrowserRuntime>(
    runtime: &R,
    script: &str,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if evaluate_bool(runtime, script).await? {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return Err(PostpubError::External(
                "timed out waiting for wechat page state".to_string(),
            ));
        }

        sleep(Duration::from_millis(500)).await;
    }
}

async fn evaluate_bool<R: BrowserRuntime>(runtime: &R, script: &str) -> Result<bool> {
    Ok(runtime.evaluate(script).await?.trim() == "true")
}

fn extract_first_heading(input: &str) -> Option<String> {
    let heading = Regex::new(r"(?m)<h1[^>]*>\s*(.*?)\s*</h1>").ok()?;
    if let Some(captures) = heading.captures(input) {
        return Some(
            strip_html_tags(captures.get(1)?.as_str())
                .trim()
                .to_string(),
        );
    }

    let markdown = Regex::new(r"(?m)^#\s+(.+?)\s*$").ok()?;
    markdown
        .captures(input)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().trim().to_string())
}

fn strip_html_tags(input: &str) -> String {
    let tags = Regex::new(r"<[^>]+>").expect("html tag regex");
    tags.replace_all(input, "").to_string()
}

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name)
            .ok()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use chrono::Utc;
    use postpub_types::{ArticleSummary, ArticleVariantSummary, WechatPublishTargetConfig};
    use tempfile::tempdir;

    use super::*;

    #[derive(Clone, Default)]
    struct MockRuntime {
        calls: Arc<Mutex<Vec<String>>>,
        eval_results: Arc<Mutex<VecDeque<String>>>,
        texts: Arc<Mutex<VecDeque<String>>>,
        urls: Arc<Mutex<VecDeque<String>>>,
    }

    impl MockRuntime {
        fn with_eval_results(results: &[&str]) -> Self {
            Self {
                eval_results: Arc::new(Mutex::new(
                    results.iter().map(|item| item.to_string()).collect(),
                )),
                ..Self::default()
            }
        }
    }

    #[async_trait]
    impl BrowserRuntime for MockRuntime {
        async fn open(&self, url: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("open:{url}"));
            Ok(())
        }

        async fn click(&self, selector: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("click:{selector}"));
            Ok(())
        }

        async fn fill(&self, selector: &str, text: &str) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("fill:{selector}:{text}"));
            Ok(())
        }

        async fn upload(&self, selector: &str, files: &[String]) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("upload:{selector}:{}", files.join(",")));
            Ok(())
        }

        async fn get_url(&self) -> Result<String> {
            Ok(self.urls.lock().unwrap().pop_front().unwrap_or_else(|| {
                "https://mp.weixin.qq.com/cgi-bin/home?t=home/index&lang=zh_CN&token=123"
                    .to_string()
            }))
        }

        async fn get_text(&self, _selector: &str) -> Result<String> {
            Ok(self.texts.lock().unwrap().pop_front().unwrap_or_default())
        }

        async fn evaluate(&self, _script: &str) -> Result<String> {
            Ok(self
                .eval_results
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| "true".to_string()))
        }

        async fn wait_load(&self, _state: &str) -> Result<()> {
            Ok(())
        }

        async fn wait_ms(&self, _ms: u64) -> Result<()> {
            Ok(())
        }
    }

    fn sample_article(relative_path: &str) -> ArticleDocument {
        ArticleDocument {
            summary: ArticleSummary {
                name: "demo".to_string(),
                title: "Demo Title".to_string(),
                relative_path: relative_path.to_string(),
                format: "MD".to_string(),
                size_bytes: 0,
                updated_at: Utc::now(),
                status: "draft".to_string(),
                variant_count: 1,
            },
            content: "# Demo Title\n\nBody".to_string(),
            preview_html: "<h1>Demo Title</h1><p>Body</p>".to_string(),
            variants: Vec::new(),
        }
    }

    fn sample_variant() -> ArticleVariantDocument {
        ArticleVariantDocument {
            summary: ArticleVariantSummary {
                target_id: "publish-wechat-1".to_string(),
                target_name: "WeChat".to_string(),
                platform_type: "wechat".to_string(),
                format: "HTML".to_string(),
                size_bytes: 0,
                updated_at: Utc::now(),
            },
            content: "<h1>Demo Title</h1><p>Body</p>".to_string(),
            preview_html: "<h1>Demo Title</h1><p>Body</p>".to_string(),
        }
    }

    fn sample_target() -> PublishTargetConfig {
        PublishTargetConfig {
            wechat: WechatPublishTargetConfig {
                cover_strategy: "custom_path".to_string(),
                cover_path: "cover.png".to_string(),
                ..WechatPublishTargetConfig::default()
            },
            ..PublishTargetConfig::default()
        }
    }

    #[test]
    fn resolves_custom_cover_path_from_images_dir() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        context.bootstrap().expect("bootstrap");
        std::fs::write(context.paths().images_dir().join("cover.png"), b"cover").expect("write");

        let article = sample_article("demo.md");
        let target = sample_target();
        let path = resolve_wechat_cover_path(&context, &target, &article).expect("cover path");
        assert!(path.ends_with("cover.png"));
    }

    #[tokio::test]
    async fn publish_fails_fast_when_cover_is_missing() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        context.bootstrap().expect("bootstrap");

        let publisher = WechatPublisher::new(context);
        let runtime = MockRuntime::with_eval_results(&[]);
        let reporter = PublishProgressReporter::new(|_, _| {});
        let result = publisher
            .publish_with_runtime(
                &runtime,
                &sample_target(),
                &sample_article("demo.md"),
                &sample_variant(),
                &PublishArticleRequest {
                    article_relative_path: "demo.md".to_string(),
                    target_id: "publish-wechat-1".to_string(),
                    mode: "draft".to_string(),
                },
                &reporter,
            )
            .await;

        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("cover"));
    }

    #[test]
    fn rejects_reward_setting_until_it_is_supported() {
        let mut target = sample_target();
        target.wechat.enable_reward = true;

        let result = validate_wechat_supported_settings(&target);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("reward"));
    }

    #[test]
    fn rejects_collection_setting_until_it_is_supported() {
        let mut target = sample_target();
        target.wechat.collection_id = "collection-1".to_string();

        let result = validate_wechat_supported_settings(&target);
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("collection"));
    }

    #[test]
    fn builds_isolated_browser_profile_path_from_target_id() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        let mut target = sample_target();
        target.id = "Wechat Target / Prod".to_string();

        let path = resolve_browser_profile_dir(&context, &target).expect("profile path");
        assert!(path.ends_with("wechat-target-prod"));
    }
}
