use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use async_trait::async_trait;
use postpub_types::{
    ArticleDesign, ArticleDocument, ArticleVariantDocument, PublishArticleRequest, PublishOutput,
    PublishTargetConfig,
};
use regex::Regex;
use tokio::{
    process::Command,
    time::{sleep, timeout, Instant},
};

use crate::browser::{browser_profile_dir, sanitize_profile_component};
use crate::{preview_html, AppContext, PostpubError, Result};

use super::{runtime::AgentBrowserRuntime, BrowserRuntime, PublishProgressReporter, Publisher};
#[derive(Clone)]
pub struct WechatPublisher {
    context: AppContext,
}

#[derive(Debug, Clone)]
enum WechatCoverPlan {
    Upload(PathBuf),
    BodyFirstImage,
    PlatformAi,
}

#[derive(Debug, Clone, Copy)]
struct WechatCoverSize {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct WechatCoverRequest {
    plan: WechatCoverPlan,
    size: WechatCoverSize,
}

#[derive(Debug, Clone)]
struct WechatPublishPlan {
    title: String,
    body_html: String,
    format: String,
    cover: WechatCoverRequest,
}

impl WechatCoverPlan {
    fn description(&self) -> &'static str {
        match self {
            Self::Upload(_) => "本地文件上传",
            Self::BodyFirstImage => "从正文选择首图",
            Self::PlatformAi => "公众号 AI 配图",
        }
    }
}

impl WechatPublisher {
    pub fn new(context: AppContext) -> Self {
        Self { context }
    }

    async fn publish_with_runtime<R: BrowserRuntime>(
        &self,
        runtime: &R,
        target: &PublishTargetConfig,
        plan: &WechatPublishPlan,
        request: &PublishArticleRequest,
        reporter: &PublishProgressReporter,
    ) -> Result<PublishOutput> {
        if request.mode.trim().eq_ignore_ascii_case("publish") {
            return Err(PostpubError::Validation(
                "wechat publish mode is not implemented yet; use draft mode first".to_string(),
            ));
        }

        let origin = wechat_origin(&target.publish_url)?;

        reporter.report("wechat.browser", "starting agent-browser session");
        reporter.report("wechat.browser.open", "opening wechat dashboard");
        open_wechat_dashboard(runtime, &target.publish_url, reporter).await?;

        reporter.report("wechat.auth", "checking wechat login state");
        let current_url = runtime.get_url().await?;
        reporter.report("wechat.auth.url", format!("current page {current_url}"));
        let Some(token) = extract_query_value(&current_url, "token") else {
            let body_text = runtime.get_text("body").await?;
            if body_text.contains("使用账号登录") || body_text.contains("登录") {
                return Err(PostpubError::External(format!(
                    "wechat login is required. Please log in once with the browser profile for target '{}'",
                    target.id
                )));
            }
            return Err(PostpubError::External(format!(
                "failed to determine wechat token from current url: {current_url}"
            )));
        };

        open_wechat_editor(runtime, &origin, &token, reporter).await?;

        reporter.report("wechat.title", "填写文章标题");
        runtime.fill(".js_title", &plan.title).await?;

        if !target.account_name.trim().is_empty() {
            reporter.report("wechat.author", "填写作者信息");
            runtime
                .fill(".js_author", target.account_name.trim())
                .await?;
        }

        reporter.report("wechat.body", "写入文章正文");
        set_editor_html(runtime, &plan.body_html).await?;
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
        click_first_visible_selector(
            runtime,
            &[
                "#js_cover_area .js_cover_btn_area",
                "#js_cover_area #js_cover_null",
                "#js_cover_area",
            ],
        )
        .await?;
        wait_for_any_visible_selector(
            runtime,
            &[
                "#js_cover_area #js_cover_null .js_imagedialog",
                "#js_cover_area .js_cover_opr .js_imagedialog",
            ],
            Duration::from_secs(10),
        )
        .await?;

        apply_wechat_cover(runtime, &plan.cover, reporter).await?;

        apply_wechat_publish_settings(runtime, target, reporter).await?;

        reporter.report("wechat.save", "保存为草稿");
        click_by_text(runtime, "保存为草稿", true).await?;
        wait_for_text(runtime, "手动保存", Duration::from_secs(20)).await?;

        let remote_url = Some(runtime.get_url().await?);
        let remote_id = remote_url
            .as_deref()
            .and_then(|url| extract_query_value(url, "appmsgid"));

        reporter.report("wechat.done", "微信公众号草稿保存完成");
        Ok(PublishOutput {
            article_relative_path: request.article_relative_path.clone(),
            article_title: plan.title.clone(),
            target_id: target.id.clone(),
            target_name: target.name.clone(),
            platform_type: target.platform_type.clone(),
            mode: request.mode.clone(),
            format: plan.format.clone(),
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

        let plan = build_wechat_publish_plan(&self.context, target, article, variant)?;

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
        cleanup_wechat_browser_runtime(target, &browser_executable, &browser_profile, reporter)
            .await?;
        reporter.report(
            "wechat.browser.profile",
            format!(
                "using isolated browser profile {}",
                browser_profile.display()
            ),
        );
        let runtime = AgentBrowserRuntime::new(
            resolve_wechat_session_name(target),
            Some(browser_executable),
            Some(browser_profile),
        );
        reporter.report(
            "wechat.browser.mode",
            if runtime.is_headed() {
                "使用可视浏览器窗口执行发布流程"
            } else {
                "使用 headless 模式执行发布流程"
            },
        );
        reporter.report(
            "wechat.browser.session",
            format!("using agent-browser session {}", runtime.session_name()),
        );
        let publish_result = self
            .publish_with_runtime(
                &runtime,
                target,
                &plan,
                request,
                reporter,
            )
            .await;

        reporter.report("wechat.browser.close", "closing agent-browser session");
        let close_result = runtime.close().await;
        match &close_result {
            Ok(()) => reporter.report("wechat.browser.closed", "agent-browser session closed"),
            Err(error) => reporter.report(
                "wechat.browser.close_error",
                format!("failed to close agent-browser session: {error}"),
            ),
        }

        match (publish_result, close_result) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(output), Ok(())) => Ok(output),
        }
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

fn build_wechat_publish_plan(
    context: &AppContext,
    target: &PublishTargetConfig,
    article: &ArticleDocument,
    variant: &ArticleVariantDocument,
) -> Result<WechatPublishPlan> {
    validate_wechat_supported_settings(target)?;

    let title = article_title(article, variant);
    let body_html = article_body_html(article, variant);
    let cover = resolve_wechat_cover_request(context, target, article, &body_html)?;

    Ok(WechatPublishPlan {
        title,
        body_html,
        format: variant.summary.format.clone(),
        cover,
    })
}

fn resolve_wechat_session_name(target: &PublishTargetConfig) -> String {
    if let Ok(value) = env::var("POSTPUB_AGENT_BROWSER_SESSION_NAME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    format!(
        "{}-{}",
        resolve_wechat_session_prefix(target),
        uuid::Uuid::new_v4().simple()
    )
}

fn resolve_wechat_session_prefix(target: &PublishTargetConfig) -> String {
    let suffix = sanitize_profile_component(if target.id.trim().is_empty() {
        "default"
    } else {
        target.id.trim()
    });
    format!("postpub-{}-{suffix}", target.platform_type.trim())
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

#[derive(Default)]
struct BrowserCleanupStats {
    daemon_processes_killed: usize,
    browser_processes_killed: usize,
    session_files_removed: usize,
    profile_lock_files_removed: usize,
}

async fn cleanup_wechat_browser_runtime(
    target: &PublishTargetConfig,
    browser_executable: &Path,
    browser_profile: &Path,
    reporter: &PublishProgressReporter,
) -> Result<()> {
    reporter.report(
        "wechat.browser.cleanup",
        "cleaning stale agent-browser sessions and chrome processes",
    );

    let session_prefix = resolve_wechat_session_prefix(target);
    let socket_dir = resolve_agent_browser_socket_dir();
    let mut stats = BrowserCleanupStats::default();

    stats.daemon_processes_killed =
        kill_agent_browser_session_processes(&socket_dir, &session_prefix).await?;
    stats.session_files_removed = remove_agent_browser_session_files(&socket_dir, &session_prefix)?;

    #[cfg(windows)]
    {
        stats.browser_processes_killed =
            kill_windows_browser_processes_for_profile(browser_executable, browser_profile).await?;
    }

    if stats.daemon_processes_killed > 0 || stats.browser_processes_killed > 0 {
        reporter.report(
            "wechat.browser.cleanup.wait",
            "waiting for previous browser processes to release the profile",
        );
        sleep(Duration::from_secs(2)).await;
    }

    stats.profile_lock_files_removed = cleanup_browser_profile_lock_files(browser_profile)?;
    reporter.report(
        "wechat.browser.cleanup.done",
        format!(
            "cleanup finished: killed {} stale session processes, {} stale chrome processes, removed {} session files, removed {} profile lock files",
            stats.daemon_processes_killed,
            stats.browser_processes_killed,
            stats.session_files_removed,
            stats.profile_lock_files_removed,
        ),
    );

    Ok(())
}

fn resolve_agent_browser_socket_dir() -> PathBuf {
    if let Ok(dir) = env::var("AGENT_BROWSER_SOCKET_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(dir) = env::var("XDG_RUNTIME_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("agent-browser");
        }
    }

    if let Some(home) = dirs::home_dir() {
        return home.join(".agent-browser");
    }

    env::temp_dir().join("agent-browser")
}

async fn kill_agent_browser_session_processes(
    socket_dir: &Path,
    session_prefix: &str,
) -> Result<usize> {
    if !socket_dir.exists() {
        return Ok(0);
    }

    let mut killed = 0;
    for entry in fs::read_dir(socket_dir)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(session_name) = file_name.strip_suffix(".pid") else {
            continue;
        };
        if !session_name.starts_with(session_prefix) {
            continue;
        }

        let pid = fs::read_to_string(entry.path())
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok());
        let Some(pid) = pid else {
            continue;
        };

        if terminate_process_tree(pid).await? {
            killed += 1;
        }
    }

    Ok(killed)
}

async fn terminate_process_tree(pid: u32) -> Result<bool> {
    #[cfg(windows)]
    {
        let output = timeout(
            Duration::from_secs(10),
            Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F", "/T"])
                .output(),
        )
        .await
        .map_err(|_| {
            PostpubError::External(format!(
                "timed out while terminating stale process tree: {pid}"
            ))
        })??;
        return Ok(output.status.success());
    }

    #[cfg(not(windows))]
    {
        let output = timeout(
            Duration::from_secs(10),
            Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .output(),
        )
        .await
        .map_err(|_| {
            PostpubError::External(format!(
                "timed out while terminating stale process tree: {pid}"
            ))
        })??;
        Ok(output.status.success())
    }
}

fn remove_agent_browser_session_files(socket_dir: &Path, session_prefix: &str) -> Result<usize> {
    if !socket_dir.exists() {
        return Ok(0);
    }

    const SESSION_SUFFIXES: &[&str] = &[
        ".pid",
        ".port",
        ".version",
        ".stream",
        ".engine",
        ".provider",
        ".extensions",
        ".sock",
    ];

    let mut removed = 0;
    for entry in fs::read_dir(socket_dir)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        let matches_prefix = file_name.starts_with(session_prefix);
        let matches_suffix = SESSION_SUFFIXES
            .iter()
            .any(|suffix| file_name.ends_with(suffix));
        if !matches_prefix || !matches_suffix {
            continue;
        }

        let path = entry.path();
        let removed_now = if path.is_dir() {
            fs::remove_dir_all(&path).is_ok()
        } else {
            fs::remove_file(&path).is_ok()
        };
        if removed_now {
            removed += 1;
        }
    }

    Ok(removed)
}

#[cfg(windows)]
async fn kill_windows_browser_processes_for_profile(
    browser_executable: &Path,
    browser_profile: &Path,
) -> Result<usize> {
    let executable = browser_executable
        .canonicalize()
        .ok()
        .unwrap_or_else(|| browser_executable.to_path_buf());
    let profile = browser_profile
        .canonicalize()
        .ok()
        .unwrap_or_else(|| browser_profile.to_path_buf());
    let executable = normalize_windows_process_match_path(&executable);
    let profile = normalize_windows_process_match_path(&profile);

    let script = r#"
$browserPath = $env:POSTPUB_BROWSER_EXECUTABLE
$profilePath = $env:POSTPUB_BROWSER_PROFILE
$matched = @(
  Get-CimInstance Win32_Process | Where-Object {
    $_.Name -eq 'chrome.exe' -and
    $_.ExecutablePath -and
    ($_.ExecutablePath -ieq $browserPath) -and
    $_.CommandLine -and
    ($_.CommandLine -match [regex]::Escape($profilePath))
  }
)
$ids = @($matched | Select-Object -ExpandProperty ProcessId)
foreach ($id in $ids) {
  Stop-Process -Id $id -Force -ErrorAction SilentlyContinue
}
Write-Output $ids.Count
"#;

    let mut command = Command::new("powershell");
    command
        .arg("-NoProfile")
        .arg("-Command")
        .arg(script)
        .env("POSTPUB_BROWSER_EXECUTABLE", executable)
        .env("POSTPUB_BROWSER_PROFILE", profile)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = timeout(Duration::from_secs(20), command.output())
        .await
        .map_err(|_| {
            PostpubError::External("timed out while terminating stale chrome processes".to_string())
        })??;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        let detail = if stderr.is_empty() { stdout } else { stderr };
        return Err(PostpubError::External(format!(
            "failed to terminate stale chrome processes: {detail}"
        )));
    }

    Ok(stdout.parse::<usize>().unwrap_or(0))
}

#[cfg(not(windows))]
async fn kill_windows_browser_processes_for_profile(
    _browser_executable: &Path,
    _browser_profile: &Path,
) -> Result<usize> {
    Ok(0)
}

fn cleanup_browser_profile_lock_files(browser_profile: &Path) -> Result<usize> {
    if !browser_profile.exists() {
        return Ok(0);
    }

    const PROFILE_LOCK_FILES: &[&str] = &[
        "DevToolsActivePort",
        "lockfile",
        "SingletonLock",
        "SingletonCookie",
        "SingletonSocket",
    ];

    let mut removed = 0;
    for name in PROFILE_LOCK_FILES {
        let path = browser_profile.join(name);
        if !path.exists() {
            continue;
        }

        let removed_now = if path.is_dir() {
            fs::remove_dir_all(&path).is_ok()
        } else {
            fs::remove_file(&path).is_ok()
        };
        if removed_now {
            removed += 1;
        }
    }

    Ok(removed)
}

#[cfg(windows)]
fn normalize_windows_process_match_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .replace('/', "\\")
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

fn resolve_wechat_cover_request(
    context: &AppContext,
    target: &PublishTargetConfig,
    article: &ArticleDocument,
    body_html: &str,
) -> Result<WechatCoverRequest> {
    let size = resolve_wechat_cover_size(target)?;
    let body_has_image = html_contains_image(body_html);

    let plan = match target.wechat.cover_strategy.trim() {
        "custom_path" => WechatCoverPlan::Upload(resolve_wechat_cover_path(context, target, article)?),
        "article_cover" => WechatCoverPlan::Upload(resolve_wechat_cover_path(context, target, article)?),
        "first_image" if body_has_image => WechatCoverPlan::BodyFirstImage,
        "first_image" => {
            return Err(PostpubError::Validation(
                "wechat cover strategy 'first_image' requires at least one image in the article body"
                    .to_string(),
            ));
        }
        "platform_ai" => WechatCoverPlan::PlatformAi,
        "manual" => {
            return Err(PostpubError::Validation(
                "wechat cover strategy 'manual' is not supported in automated publish yet"
                    .to_string(),
            ));
        }
        other => {
            return Err(PostpubError::Validation(format!(
                "unsupported wechat cover strategy: {other}"
            )));
        }
    };

    Ok(WechatCoverRequest { plan, size })
}

fn resolve_wechat_cover_size(target: &PublishTargetConfig) -> Result<WechatCoverSize> {
    let width = target.wechat.cover_width;
    let height = target.wechat.cover_height;
    if width == 0 || height == 0 {
        return Err(PostpubError::Validation(format!(
            "invalid wechat cover size {}x{}",
            width, height
        )));
    }

    Ok(WechatCoverSize { width, height })
}

fn resolve_wechat_cover_path(
    context: &AppContext,
    target: &PublishTargetConfig,
    article: &ArticleDocument,
) -> Result<PathBuf> {
    let Some(raw_path) = resolve_wechat_cover_source(context, target, article) else {
        return Err(PostpubError::Validation(
            "wechat cover image is not configured. Set a local cover path or save an article cover first"
                .to_string(),
        ));
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

fn resolve_wechat_cover_source(
    context: &AppContext,
    target: &PublishTargetConfig,
    article: &ArticleDocument,
) -> Option<String> {
    let design = context
        .article_store()
        .load_article_design(&article.summary.relative_path)
        .unwrap_or_else(|_| ArticleDesign::default());

    match target.wechat.cover_strategy.trim() {
        "custom_path" => Some(target.wechat.cover_path.trim().to_string()),
        "article_cover" => {
            if !design.cover.trim().is_empty() {
                Some(design.cover.trim().to_string())
            } else if !target.wechat.cover_path.trim().is_empty() {
                Some(target.wechat.cover_path.trim().to_string())
            } else {
                None
            }
        }
        _ => None,
    }
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

async fn open_wechat_editor<R: BrowserRuntime>(
    runtime: &R,
    origin: &str,
    token: &str,
    reporter: &PublishProgressReporter,
) -> Result<()> {
    let editor_url = format!(
        "{origin}/cgi-bin/appmsg?t=media/appmsg_edit_v2&action=edit&isNew=1&type=77&token={token}"
    );
    reporter.report("wechat.editor.direct", "opening direct editor url");
    navigate_to_url(runtime, &editor_url).await?;
    wait_for_editor_ready(runtime, Duration::from_secs(15)).await?;
    reporter.report("wechat.editor.ready", "direct editor page is ready");
    Ok(())
}

async fn open_wechat_dashboard<R: BrowserRuntime>(
    runtime: &R,
    url: &str,
    reporter: &PublishProgressReporter,
) -> Result<()> {
    open_url(runtime, url, 5_000).await?;
    reporter.report("wechat.browser.opened", "wechat dashboard opened");
    reporter.report(
        "wechat.browser.wait",
        "waiting for wechat dashboard to become interactive",
    );

    let current_url = wait_for_wechat_dashboard_state(runtime, Duration::from_secs(20)).await?;
    reporter.report(
        "wechat.browser.ready",
        if current_url.is_empty() {
            "wechat dashboard is ready".to_string()
        } else {
            format!("wechat dashboard is ready at {current_url}")
        },
    );

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
        click_any_text(runtime, &["确定", "确认", "保存"], true).await?;
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
        click_any_text(runtime, &["确定", "确认", "保存"], true).await?;
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

async fn apply_wechat_cover<R: BrowserRuntime>(
    runtime: &R,
    cover: &WechatCoverRequest,
    reporter: &PublishProgressReporter,
) -> Result<()> {
    reporter.report(
        "wechat.cover.size",
        format!(
            "targeting wechat cover size {}x{}",
            cover.size.width, cover.size.height
        ),
    );
    reporter.report(
        "wechat.cover.method",
        match &cover.plan {
            WechatCoverPlan::Upload(path) => {
                format!(
                    "通过{}设置封面：{}",
                    cover.plan.description(),
                    path.display()
                )
            }
            _ => format!("通过{}设置封面", cover.plan.description()),
        },
    );

    ensure_wechat_cover_picker_open(runtime).await?;

    match &cover.plan {
        WechatCoverPlan::Upload(path) => upload_wechat_cover_file(runtime, path).await?,
        WechatCoverPlan::BodyFirstImage => {
            select_wechat_cover_from_article(runtime, cover.size).await?
        }
        WechatCoverPlan::PlatformAi => select_wechat_cover_with_ai(runtime, cover.size).await?,
    }

    wait_for_wechat_cover_preview(runtime, Duration::from_secs(20)).await?;
    reporter.report(
        "wechat.cover.ready",
        format!("封面已通过{}设置完成", cover.plan.description()),
    );
    Ok(())
}

async fn click_by_text<R: BrowserRuntime>(runtime: &R, text: &str, exact: bool) -> Result<()> {
    let matcher = serde_json::to_string(text)?;
    let script = format!(
        r#"
(() => {{
  const expected = {matcher};
  const exact = {exact};
  const normalize = (input) => (input || "").replace(/\s+/g, " ").trim();
  const isVisible = (node) => {{
    if (!node) {{
      return false;
    }}
    const rect = node.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }};
  const rank = (node) => {{
    const tag = (node.tagName || "").toLowerCase();
    if (tag === "button" || tag === "a") {{
      return 0;
    }}
    if ((node.getAttribute("role") || "").toLowerCase() === "button") {{
      return 1;
    }}
    if (node.hasAttribute("tabindex")) {{
      return 2;
    }}
    if (tag === "li") {{
      return 3;
    }}
    return 4;
  }};
  const seen = new Set();
  const nodes = Array.from(document.querySelectorAll("a,button,div,li,span"));
  const candidate = nodes
    .filter((node) => {{
      const rect = node.getBoundingClientRect();
      if (rect.width <= 0 || rect.height <= 0) {{
        return false;
      }}
      const value = normalize(node.innerText || node.textContent || "");
      return exact ? value === expected : value.includes(expected);
    }})
    .map((node) => {{
      const target =
        node.closest('button,a,[role="button"],[tabindex],li') || node;
      if (!isVisible(target)) {{
        return null;
      }}
      const value = normalize(target.innerText || target.textContent || node.innerText || node.textContent || "");
      if (seen.has(target)) {{
        return null;
      }}
      seen.add(target);
      const rect = target.getBoundingClientRect();
      return {{
        target,
        value,
        rank: rank(target),
        area: rect.width * rect.height,
      }};
    }})
    .filter(Boolean)
    .sort((left, right) => {{
      return left.rank - right.rank || left.value.length - right.value.length || right.area - left.area;
    }})[0];
  if (!candidate) {{
    return false;
  }}
  const target = candidate.target;
  target.scrollIntoView({{ block: "center", inline: "center" }});
  for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup", "click"]) {{
    target.dispatchEvent(
      new MouseEvent(type, {{ bubbles: true, cancelable: true, view: window }})
    );
  }}
  if (typeof target.click === "function") {{
    target.click();
  }}
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
    let mut failures = Vec::new();
    for candidate in candidates {
        match click_by_text(runtime, candidate, exact).await {
            Ok(()) => return Ok(()),
            Err(error) => failures.push(format!("{candidate}: {error}")),
        }
    }

    Err(PostpubError::External(format!(
        "failed to click any wechat text target: {}",
        failures.join(" | ")
    )))
}

async fn open_url<R: BrowserRuntime>(runtime: &R, url: &str, timeout_ms: u64) -> Result<()> {
    runtime.open_with_timeout_ms(url, timeout_ms).await
}

async fn navigate_to_url<R: BrowserRuntime>(runtime: &R, url: &str) -> Result<()> {
    let url_json = serde_json::to_string(url)?;
    let script = format!(
        r#"
(() => {{
  window.location.assign({url_json});
  return window.location.href;
}})()
"#
    );

    runtime.evaluate(&script).await.map(|_| ())
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

async fn click_first_visible_selector<R: BrowserRuntime>(
    runtime: &R,
    selectors: &[&str],
) -> Result<()> {
    let selectors_json = serde_json::to_string(selectors)?;
    let script = format!(
        r#"
(() => {{
  const selectors = {selectors_json};
  const isVisible = (node) => {{
    if (!node) {{
      return false;
    }}
    const rect = node.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {{
      return false;
    }}
    const style = getComputedStyle(node);
    return style.display !== "none" && style.visibility !== "hidden";
  }};

  for (const selector of selectors) {{
    const target = Array.from(document.querySelectorAll(selector)).find(isVisible);
    if (!target) {{
      continue;
    }}
    target.scrollIntoView({{ block: "center", inline: "center" }});
    for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup", "click"]) {{
      target.dispatchEvent(
        new MouseEvent(type, {{ bubbles: true, cancelable: true, view: window }})
      );
    }}
    if (typeof target.click === "function") {{
      target.click();
    }}
    return true;
  }}

  return false;
}})()
"#
    );

    if evaluate_bool(runtime, &script).await? {
        Ok(())
    } else {
        Err(PostpubError::External(format!(
            "failed to click visible wechat selector: {}",
            selectors.join(", ")
        )))
    }
}

async fn wait_for_any_visible_selector<R: BrowserRuntime>(
    runtime: &R,
    selectors: &[&str],
    timeout: Duration,
) -> Result<()> {
    let selectors_json = serde_json::to_string(selectors)?;
    let script = format!(
        r#"
(() => {{
  const selectors = {selectors_json};
  const isVisible = (node) => {{
    if (!node) {{
      return false;
    }}
    const rect = node.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {{
      return false;
    }}
    const style = getComputedStyle(node);
    return style.display !== "none" && style.visibility !== "hidden";
  }};

  return selectors.some((selector) =>
    Array.from(document.querySelectorAll(selector)).some(isVisible)
  );
}})()
"#
    );

    wait_until_true(runtime, &script, timeout).await
}

async fn is_any_visible_selector<R: BrowserRuntime>(
    runtime: &R,
    selectors: &[&str],
) -> Result<bool> {
    let selectors_json = serde_json::to_string(selectors)?;
    let script = format!(
        r#"
(() => {{
  const selectors = {selectors_json};
  const isVisible = (node) => {{
    if (!node) {{
      return false;
    }}
    const rect = node.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {{
      return false;
    }}
    const style = getComputedStyle(node);
    return style.display !== "none" && style.visibility !== "hidden";
  }};

  return selectors.some((selector) =>
    Array.from(document.querySelectorAll(selector)).some(isVisible)
  );
}})()
"#
    );

    evaluate_bool(runtime, &script).await
}

async fn ensure_wechat_cover_picker_open<R: BrowserRuntime>(runtime: &R) -> Result<()> {
    if is_any_visible_selector(
        runtime,
        &[
            ".weui-desktop-dialog_img-picker",
            ".weui-desktop-dialog",
            ".weui-desktop-popover",
        ],
    )
    .await?
    {
        return Ok(());
    }

    click_first_visible_selector(
        runtime,
        &[
            "#js_cover_area #js_cover_null .js_imagedialog",
            "#js_cover_area .js_cover_opr .js_imagedialog",
        ],
    )
    .await?;
    wait_for_text(runtime, "选择图片", Duration::from_secs(10)).await
}

async fn upload_wechat_cover_file<R: BrowserRuntime>(runtime: &R, cover_path: &Path) -> Result<()> {
    runtime
        .upload(
            r#".weui-desktop-dialog_img-picker .weui-desktop-upload input[type="file"]"#,
            &[cover_path.to_string_lossy().to_string()],
        )
        .await?;

    let file_name = cover_path
        .file_name()
        .map(|item| item.to_string_lossy().to_string())
        .ok_or_else(|| PostpubError::InvalidPath(cover_path.display().to_string()))?;
    wait_for_text(runtime, &file_name, Duration::from_secs(20)).await
}

async fn select_wechat_cover_from_article<R: BrowserRuntime>(
    runtime: &R,
    _cover_size: WechatCoverSize,
) -> Result<()> {
    click_any_text(
        runtime,
        &["从正文选择", "正文图片", "正文", "从正文选取"],
        false,
    )
    .await?;
    wait_for_wechat_cover_candidates(runtime, Duration::from_secs(10)).await?;
    click_first_wechat_cover_candidate(runtime).await
}

async fn select_wechat_cover_with_ai<R: BrowserRuntime>(
    runtime: &R,
    _cover_size: WechatCoverSize,
) -> Result<()> {
    click_any_text(runtime, &["AI配图", "AI 配图", "智能配图"], false).await?;
    click_any_text(
        runtime,
        &["生成封面", "开始生成", "立即生成", "生成配图", "生成"],
        false,
    )
    .await?;
    wait_for_wechat_cover_candidates(runtime, Duration::from_secs(30)).await?;
    click_first_wechat_cover_candidate(runtime).await
}

async fn wait_for_wechat_cover_candidates<R: BrowserRuntime>(
    runtime: &R,
    timeout: Duration,
) -> Result<()> {
    let script = r#"
(() => {
  const normalize = (input) => (input || "").replace(/\s+/g, " ").trim();
  const isVisible = (node) => {
    if (!node) {
      return false;
    }
    const rect = node.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      return false;
    }
    const style = getComputedStyle(node);
    return style.display !== "none" && style.visibility !== "hidden";
  };
  const excludedTexts = ["上传", "本地上传", "选择图片", "从正文选择", "AI配图", "AI 配图", "智能配图", "下一步", "确定", "确认", "取消", "关闭", "保存"];
  const dialog =
    Array.from(document.querySelectorAll(".weui-desktop-dialog, .weui-desktop-dialog_img-picker, .weui-desktop-popover"))
      .filter(isVisible)
      .pop() || document.body;
  const nodes = Array.from(dialog.querySelectorAll("button, label, li, div, a, span, img"));
  return nodes.some((node) => {
    const target = node.closest("button, label, li, div, a") || node;
    if (!isVisible(target)) {
      return false;
    }
    const text = normalize(target.innerText || target.textContent || "");
    if (excludedTexts.some((item) => text.includes(item))) {
      return false;
    }
    const hasImage = !!target.querySelector("img");
    const style = getComputedStyle(target);
    return hasImage || (style.backgroundImage && style.backgroundImage !== "none");
  });
})()
"#;

    wait_until_true(runtime, script, timeout).await
}

async fn click_first_wechat_cover_candidate<R: BrowserRuntime>(runtime: &R) -> Result<()> {
    let script = r#"
(() => {
  const normalize = (input) => (input || "").replace(/\s+/g, " ").trim();
  const isVisible = (node) => {
    if (!node) {
      return false;
    }
    const rect = node.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      return false;
    }
    const style = getComputedStyle(node);
    return style.display !== "none" && style.visibility !== "hidden";
  };
  const excludedTexts = ["上传", "本地上传", "选择图片", "从正文选择", "AI配图", "AI 配图", "智能配图", "下一步", "确定", "确认", "取消", "关闭", "保存"];
  const dialog =
    Array.from(document.querySelectorAll(".weui-desktop-dialog, .weui-desktop-dialog_img-picker, .weui-desktop-popover"))
      .filter(isVisible)
      .pop() || document.body;
  const candidate = Array.from(dialog.querySelectorAll("button, label, li, div, a, span, img"))
    .map((node) => node.closest("button, label, li, div, a") || node)
    .filter((node) => {
      if (!isVisible(node)) {
        return false;
      }
      const text = normalize(node.innerText || node.textContent || "");
      if (excludedTexts.some((item) => text.includes(item))) {
        return false;
      }
      const hasImage = !!node.querySelector("img");
      const style = getComputedStyle(node);
      return hasImage || (style.backgroundImage && style.backgroundImage !== "none");
    })
    .map((node) => {
      const rect = node.getBoundingClientRect();
      const text = normalize(node.innerText || node.textContent || "");
      return { node, textLength: text.length, area: rect.width * rect.height };
    })
    .sort((left, right) => left.textLength - right.textLength || right.area - left.area)[0];

  if (!candidate) {
    return false;
  }

  const target = candidate.node;
  target.scrollIntoView({ block: "center", inline: "center" });
  for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup", "click"]) {
    target.dispatchEvent(
      new MouseEvent(type, { bubbles: true, cancelable: true, view: window })
    );
  }
  if (typeof target.click === "function") {
    target.click();
  }
  return true;
})()
"#;

    if evaluate_bool(runtime, script).await? {
        Ok(())
    } else {
        Err(PostpubError::External(
            "failed to select a wechat cover candidate".to_string(),
        ))
    }
}

async fn wait_for_wechat_cover_preview<R: BrowserRuntime>(
    runtime: &R,
    timeout: Duration,
) -> Result<()> {
    wait_until_true(
        runtime,
        r##"(() => {
            const preview = document.querySelector("#js_cover_area .js_cover_preview_new");
            return !!preview && getComputedStyle(preview).backgroundImage && getComputedStyle(preview).backgroundImage !== "none";
        })()"##,
        timeout,
    )
    .await
}

async fn wait_for_wechat_dashboard_state<R: BrowserRuntime>(
    runtime: &R,
    timeout: Duration,
) -> Result<String> {
    let dashboard_ready_script = r##"
(() => {
  const ready = document.readyState === "interactive" || document.readyState === "complete";
  if (!ready) {
    return false;
  }

  const text = document.body?.innerText || "";
  const selectors = [
    ".weui-desktop-layout",
    ".weui-desktop-panel",
    ".weui-desktop-menu",
    "#app"
  ];
  const hasDesktopUi = selectors.some((selector) => document.querySelector(selector));
  return hasDesktopUi
    || text.includes("公众号")
    || text.includes("内容管理")
    || text.includes("登录")
    || text.includes("首页");
})()
"##;

    let deadline = Instant::now() + timeout;
    let mut last_url = String::new();

    loop {
        let current_url = runtime.get_url().await?;
        if !current_url.trim().is_empty() {
            last_url = current_url.clone();
        }

        if looks_like_wechat_url(&current_url) && evaluate_bool(runtime, dashboard_ready_script).await?
        {
            return Ok(current_url);
        }

        if Instant::now() >= deadline {
            let mut detail =
                "timed out waiting for wechat dashboard to become interactive".to_string();
            if !last_url.is_empty() {
                detail.push_str(&format!(" (last url: {last_url})"));
            }
            return Err(PostpubError::External(detail));
        }

        sleep(Duration::from_millis(500)).await;
    }
}

async fn wait_for_text<R: BrowserRuntime>(
    runtime: &R,
    expected: &str,
    timeout: Duration,
) -> Result<()> {
    let expected_json = serde_json::to_string(expected)?;
    let inner_text_script =
        format!(r#"(() => (document.body?.innerText || "").includes({expected_json}))()"#);
    let deadline = Instant::now() + timeout;
    loop {
        let text = runtime.get_text("body").await?;
        if text.contains(expected) {
            return Ok(());
        }
        if evaluate_bool(runtime, &inner_text_script).await? {
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

fn looks_like_wechat_url(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .host_str()
                .map(|host| host.eq_ignore_ascii_case("mp.weixin.qq.com"))
        })
        .unwrap_or(false)
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

async fn wait_for_editor_ready<R: BrowserRuntime>(runtime: &R, timeout: Duration) -> Result<()> {
    wait_until_true(
        runtime,
        r#"(() => !!document.querySelector(".js_title") && !!document.querySelector(".ProseMirror"))()"#,
        timeout,
    )
    .await
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

fn html_contains_image(input: &str) -> bool {
    input.to_ascii_lowercase().contains("<img")
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use chrono::Utc;
    use postpub_types::{ArticleSummary, WechatPublishTargetConfig};
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

        async fn open_with_timeout_ms(&self, url: &str, timeout_ms: u64) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("open_timeout:{url}:{timeout_ms}"));
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

    #[test]
    fn first_image_strategy_requires_body_image() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        context.bootstrap().expect("bootstrap");

        let article = sample_article("demo.md");
        let mut target = PublishTargetConfig::default();
        target.wechat.cover_strategy = "first_image".to_string();

        let cover = resolve_wechat_cover_request(
            &context,
            &target,
            &article,
            r#"<section><img src="https://example.com/cover.png" /></section>"#,
        )
        .expect("cover request");

        assert!(matches!(cover.plan, WechatCoverPlan::BodyFirstImage));
        assert_eq!(cover.size.width, target.wechat.cover_width);
        assert_eq!(cover.size.height, target.wechat.cover_height);
    }

    #[test]
    fn article_cover_strategy_requires_local_cover() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        context.bootstrap().expect("bootstrap");

        let article = sample_article("demo.md");
        let target = PublishTargetConfig::default();

        let result = resolve_wechat_cover_request(
            &context,
            &target,
            &article,
            "<section><p>Body</p></section>",
        );

        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("cover"));
    }

    #[test]
    fn resolving_cover_request_fails_fast_when_custom_cover_is_missing() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        context.bootstrap().expect("bootstrap");

        let result = resolve_wechat_cover_request(
            &context,
            &sample_target(),
            &sample_article("demo.md"),
            "<section><p>Body</p></section>",
        );

        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("cover"));
    }

    #[test]
    fn rejects_invalid_wechat_cover_size() {
        let mut target = sample_target();
        target.wechat.cover_width = 0;

        let result = resolve_wechat_cover_size(&target);

        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("invalid wechat cover size"));
    }

    #[tokio::test]
    async fn open_wechat_dashboard_uses_short_open_timeout() {
        let runtime = MockRuntime::with_eval_results(&["true"]);
        runtime.urls.lock().unwrap().push_back(
            "https://mp.weixin.qq.com/cgi-bin/home?t=home/index&lang=zh_CN&token=123".to_string(),
        );
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured_events = events.clone();
        let reporter = PublishProgressReporter::new(move |stage, message| {
            captured_events.lock().unwrap().push((stage, message));
        });

        open_wechat_dashboard(&runtime, "https://mp.weixin.qq.com", &reporter)
            .await
            .expect("wechat dashboard should become ready");

        let calls = runtime.calls.lock().unwrap().clone();
        assert!(
            calls
                .iter()
                .any(|entry| entry == "open_timeout:https://mp.weixin.qq.com:5000"),
            "expected short open timeout for dashboard bootstrap, got: {calls:?}"
        );
        let recorded = events.lock().unwrap().clone();
        assert!(
            recorded
                .iter()
                .any(|(stage, _)| stage == "wechat.browser.ready"),
            "expected ready event, got: {recorded:?}"
        );
    }

    #[tokio::test]
    async fn wait_for_wechat_dashboard_state_reports_last_url_on_timeout() {
        let runtime = MockRuntime::with_eval_results(&["false"]);
        runtime.urls.lock().unwrap().push_back(
            "https://mp.weixin.qq.com/cgi-bin/home?t=home/index&lang=zh_CN&token=321".to_string(),
        );

        let error = wait_for_wechat_dashboard_state(&runtime, Duration::from_millis(0))
            .await
            .expect_err("dashboard wait should time out");

        let message = error.to_string();
        assert!(message.contains("wechat dashboard"));
        assert!(message.contains("token=321"));
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
