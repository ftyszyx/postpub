use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use async_trait::async_trait;
use chrono::Utc;
use postpub_types::{
    ArticleDesign, ArticleDocument, ArticleVariantDocument, PublishArticleRequest, PublishOutput,
    PublishTargetConfig, PublishTargetLoginStatus,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    process::Command,
    time::{sleep, timeout, Instant},
};

use crate::browser::{browser_profile_dir, sanitize_profile_component};
use crate::{preview_html, AppContext, PostpubError, Result};

use super::{runtime::AgentBrowserRuntime, BrowserRuntime, PublishProgressReporter, Publisher};

const WECHAT_LOGIN_WAIT_TIMEOUT: Duration = Duration::from_secs(120);
const WECHAT_LOGIN_POLL_INTERVAL: Duration = Duration::from_secs(1);
const WECHAT_NAVIGATION_DOM_READY_TIMEOUT_MS: u64 = 3_000;

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
    digest: String,
    format: String,
    cover: WechatCoverRequest,
}

#[derive(Debug, Clone)]
struct WechatDraftState {
    appmsgid: String,
    data_seq: String,
    content_html: String,
    cover: Option<WechatCoverState>,
}

#[derive(Debug, Clone)]
struct WechatCoverState {
    cdn_url: String,
    cdn_235_1_url: String,
    cdn_1_1_url: String,
    original_url: String,
    crop_list: String,
    fileid: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WechatBaseResp {
    ret: Option<Value>,
    err_msg: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WechatDraftApiResponse {
    #[serde(default)]
    base_resp: Option<WechatBaseResp>,
    #[serde(default)]
    ret: Option<Value>,
    #[serde(default, rename = "appMsgId")]
    app_msg_id: Option<Value>,
    #[serde(default, rename = "AppMsgId")]
    app_msg_id_alt: Option<Value>,
    #[serde(default)]
    data_seq: Option<Value>,
    #[serde(default)]
    filter_content_html: Option<Vec<WechatFilteredContent>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WechatFilteredContent {
    #[serde(default)]
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct WechatCropMultiResponse {
    #[serde(default)]
    base_resp: Option<WechatBaseResp>,
    #[serde(default)]
    result: Vec<WechatCropResult>,
}

#[derive(Debug, Clone, Deserialize)]
struct WechatCropResult {
    #[serde(default)]
    cdnurl: String,
    #[serde(default)]
    file_id: Value,
}

#[derive(Debug, Clone, Deserialize)]
struct WechatMassSendPageResponse {
    #[serde(default)]
    base_resp: Option<WechatBaseResp>,
    #[serde(default)]
    operation_seq: String,
    #[serde(default)]
    mass_send_left: Option<i64>,
    #[serde(default)]
    strategy_info: String,
    #[serde(default)]
    scene_replace: String,
}

impl WechatPublisher {
    pub fn new(context: AppContext) -> Self {
        Self { context }
    }

    pub async fn check_login_status(
        &self,
        target: &PublishTargetConfig,
    ) -> Result<PublishTargetLoginStatus> {
        let browser_executable = self
            .context
            .browser_manager()
            .ensure_browser_executable()
            .await?;
        let browser_profile = resolve_browser_profile_dir(&self.context, target)?;
        fs::create_dir_all(&browser_profile)?;

        let runtime = AgentBrowserRuntime::new(
            resolve_wechat_session_name(target),
            Some(browser_executable),
            Some(browser_profile),
        );
        let check_result = self.check_login_status_with_runtime(&runtime, target).await;
        let close_result = runtime.close().await;

        match (check_result, close_result) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(status), Ok(())) => Ok(status),
        }
    }

    async fn check_login_status_with_runtime<R: BrowserRuntime>(
        &self,
        runtime: &R,
        target: &PublishTargetConfig,
    ) -> Result<PublishTargetLoginStatus> {
        let reporter = PublishProgressReporter::new(|_, _| {});
        open_wechat_dashboard(runtime, &target.publish_url, &reporter).await?;

        let current_url = runtime.get_url().await?;
        let checked_at = Utc::now();
        let token = extract_query_value(&current_url, "token");
        let body_text = if token.is_none() {
            runtime.get_text("body").await?
        } else {
            String::new()
        };

        let (valid, needs_login, detail) = if token.is_some() {
            (true, false, Some("微信公众号登录状态有效".to_string()))
        } else if looks_like_wechat_login_page(&current_url, &body_text) {
            (
                false,
                true,
                Some("未检测到微信公众号登录状态，请在内置浏览器中重新扫码登录。".to_string()),
            )
        } else {
            (
                false,
                false,
                Some(format!(
                    "当前页面未包含微信公众号登录 token，无法确认登录状态是否有效。页面地址：{current_url}"
                )),
            )
        };

        Ok(PublishTargetLoginStatus {
            target_id: target.id.clone(),
            target_name: if target.name.trim().is_empty() {
                target.id.clone()
            } else {
                target.name.clone()
            },
            platform_type: target.platform_type.clone(),
            valid,
            needs_login,
            checked_at,
            current_url: if current_url.trim().is_empty() {
                None
            } else {
                Some(current_url)
            },
            detail,
        })
    }

    async fn publish_with_runtime<R: BrowserRuntime>(
        &self,
        runtime: &R,
        target: &PublishTargetConfig,
        plan: &WechatPublishPlan,
        request: &PublishArticleRequest,
        reporter: &PublishProgressReporter,
    ) -> Result<PublishOutput> {
        let origin = wechat_origin(&target.publish_url)?;

        reporter.report("wechat.browser", "starting agent-browser session");
        reporter.report("wechat.browser.open", "opening wechat dashboard");
        open_wechat_dashboard(runtime, &target.publish_url, reporter).await?;

        let token = ensure_wechat_login_token(runtime, target, reporter).await?;

        reporter.report(
            "wechat.editor.bootstrap",
            "opening editor to initialize wechat web api context",
        );
        open_wechat_editor(runtime, &origin, &token, reporter).await?;
        wait_for_wechat_editor_fields(runtime, Duration::from_secs(8)).await?;

        reporter.report("wechat.api.create", "creating wechat draft through web api");
        let mut draft = create_wechat_draft_via_api(runtime, &origin, &token, target, plan).await?;

        match &plan.cover.plan {
            WechatCoverPlan::BodyFirstImage => {
                reporter.report("wechat.api.cover", "setting wechat cover through web api");
                let cover_state =
                    create_wechat_cover_state_via_api(runtime, &origin, &plan.cover, plan).await?;
                draft.cover = Some(cover_state);
                draft = update_wechat_draft_via_api(runtime, &origin, &token, target, plan, &draft)
                    .await?;
            }
            WechatCoverPlan::Upload(path) => {
                return Err(PostpubError::Validation(format!(
                    "wechat web api local cover upload is not implemented yet: {}",
                    path.display()
                )));
            }
            WechatCoverPlan::PlatformAi => {
                return Err(PostpubError::Validation(
                    "wechat web api platform AI cover generation is not implemented yet"
                        .to_string(),
                ));
            }
        }

        let remote_url = Some(format!(
            "{origin}/cgi-bin/appmsg?t=media/appmsg_edit&action=edit&reprint_confirm=0&type=77&appmsgid={}&token={token}&lang=zh_CN",
            draft.appmsgid
        ));
        let remote_id = Some(draft.appmsgid.clone());

        if request.mode.trim().eq_ignore_ascii_case("publish") {
            reporter.report(
                "wechat.api.prepublish",
                "running wechat pre-publish web api checks",
            );
            prepublish_wechat_draft_via_api(runtime, &origin, &token, &draft).await?;
        }

        reporter.report("wechat.done", "wechat draft saved through web api");
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
            .publish_with_runtime(&runtime, target, &plan, request, reporter)
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
    let digest = build_wechat_digest(&body_html);
    let cover = resolve_wechat_cover_request(context, target, article, &body_html)?;

    Ok(WechatPublishPlan {
        title,
        body_html,
        digest,
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

fn normalize_wechat_page_text(input: &str) -> String {
    input.split_whitespace().collect::<String>()
}

fn looks_like_wechat_login_page(url: &str, body_text: &str) -> bool {
    if !looks_like_wechat_url(url) {
        return false;
    }

    let normalized = normalize_wechat_page_text(body_text);
    [
        "微信扫码登录",
        "扫码登录",
        "微信扫一扫",
        "公众号平台账号登录",
        "使用账号登录",
        "使用帐号登录",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
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
        "custom_path" => resolve_wechat_custom_cover_plan(context, target, article, body_has_image),
        "article_cover" => {
            resolve_wechat_article_cover_plan(context, target, article, body_has_image)
        }
        "first_image" if body_has_image => WechatCoverPlan::BodyFirstImage,
        "first_image" => WechatCoverPlan::PlatformAi,
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

fn resolve_wechat_custom_cover_plan(
    context: &AppContext,
    target: &PublishTargetConfig,
    article: &ArticleDocument,
    body_has_image: bool,
) -> WechatCoverPlan {
    if let Some(path) = resolve_optional_wechat_cover_path(context, target, article) {
        WechatCoverPlan::Upload(path)
    } else if body_has_image {
        WechatCoverPlan::BodyFirstImage
    } else {
        WechatCoverPlan::PlatformAi
    }
}

fn resolve_wechat_article_cover_plan(
    context: &AppContext,
    target: &PublishTargetConfig,
    article: &ArticleDocument,
    body_has_image: bool,
) -> WechatCoverPlan {
    if let Some(path) = resolve_optional_wechat_cover_path(context, target, article) {
        WechatCoverPlan::Upload(path)
    } else if body_has_image {
        WechatCoverPlan::BodyFirstImage
    } else {
        WechatCoverPlan::PlatformAi
    }
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

#[cfg(test)]
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

fn resolve_optional_wechat_cover_path(
    context: &AppContext,
    target: &PublishTargetConfig,
    article: &ArticleDocument,
) -> Option<PathBuf> {
    let raw_path = resolve_wechat_cover_source(context, target, article)?;
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return None;
    }

    resolve_local_cover_path(context, article, trimmed)
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
    if target.wechat.declare_original {
        return Err(PostpubError::Validation(
            "wechat original declaration is not implemented for web api publishing yet".to_string(),
        ));
    }

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

    if !target.wechat.source_label.trim().is_empty() {
        return Err(PostpubError::Validation(
            "wechat creation source declaration is not implemented for web api publishing yet"
                .to_string(),
        ));
    }

    if !target.wechat.platform_recommendation_enabled {
        return Err(PostpubError::Validation(
            "wechat platform recommendation toggle is not implemented for web api publishing yet"
                .to_string(),
        ));
    }

    match target.wechat.comment_mode.trim() {
        "auto_selected_open" | "open_all" | "closed" => {}
        other => {
            return Err(PostpubError::Validation(format!(
                "unsupported wechat comment mode: {other}"
            )))
        }
    }

    Ok(())
}

async fn create_wechat_draft_via_api<R: BrowserRuntime>(
    runtime: &R,
    origin: &str,
    token: &str,
    target: &PublishTargetConfig,
    plan: &WechatPublishPlan,
) -> Result<WechatDraftState> {
    let url = format!(
        "{origin}/cgi-bin/operate_appmsg?t=ajax-response&sub=create&type=77&token={token}&lang=zh_CN"
    );
    let fields = build_wechat_appmsg_fields(target, plan, None);
    let response = wechat_fetch_form(runtime, &url, &fields).await?;
    let parsed: WechatDraftApiResponse = serde_json::from_str(&response)?;
    parse_wechat_draft_response(parsed, plan, None)
}

async fn update_wechat_draft_via_api<R: BrowserRuntime>(
    runtime: &R,
    origin: &str,
    token: &str,
    target: &PublishTargetConfig,
    plan: &WechatPublishPlan,
    draft: &WechatDraftState,
) -> Result<WechatDraftState> {
    let url = format!(
        "{origin}/cgi-bin/operate_appmsg?t=ajax-response&sub=update&type=77&token={token}&lang=zh_CN"
    );
    let fields = build_wechat_appmsg_fields(target, plan, Some(draft));
    let response = wechat_fetch_form(runtime, &url, &fields).await?;
    let parsed: WechatDraftApiResponse = serde_json::from_str(&response)?;
    parse_wechat_draft_response(parsed, plan, Some(draft))
}

fn build_wechat_appmsg_fields(
    target: &PublishTargetConfig,
    plan: &WechatPublishPlan,
    draft: Option<&WechatDraftState>,
) -> Vec<(String, String)> {
    let mut fields = vec![
        ("count".to_string(), "1".to_string()),
        ("title0".to_string(), plan.title.clone()),
        (
            "author0".to_string(),
            target.account_name.trim().to_string(),
        ),
        ("digest0".to_string(), plan.digest.clone()),
        (
            "content0".to_string(),
            draft
                .map(|state| state.content_html.clone())
                .unwrap_or_else(|| plan.body_html.clone()),
        ),
        ("need_open_comment0".to_string(), need_open_comment(target)),
        ("reply_flag0".to_string(), "2".to_string()),
        (
            "auto_elect_comment0".to_string(),
            auto_elect_comment(target),
        ),
        ("auto_elect_reply0".to_string(), auto_elect_comment(target)),
    ];

    if !target.wechat.source_url.trim().is_empty() {
        fields.push((
            "sourceurl0".to_string(),
            target.wechat.source_url.trim().to_string(),
        ));
    }
    if let Some(state) = draft {
        fields.extend([
            ("AppMsgId".to_string(), state.appmsgid.clone()),
            ("data_seq".to_string(), state.data_seq.clone()),
            ("operate_from".to_string(), "Chrome".to_string()),
            ("isnew".to_string(), "0".to_string()),
            ("articlenum".to_string(), "1".to_string()),
        ]);

        if let Some(cover) = &state.cover {
            fields.extend([
                ("cdn_url0".to_string(), cover.cdn_url.clone()),
                ("cdn_235_1_url0".to_string(), cover.cdn_235_1_url.clone()),
                ("cdn_1_1_url0".to_string(), cover.cdn_1_1_url.clone()),
                ("cdn_3_4_url0".to_string(), String::new()),
                ("cdn_16_9_url0".to_string(), String::new()),
                ("cdn_url_back0".to_string(), cover.original_url.clone()),
                ("crop_list0".to_string(), cover.crop_list.clone()),
                ("fileid0".to_string(), cover.fileid.clone()),
                ("last_choose_cover_from0".to_string(), "0".to_string()),
            ]);
        }
    }

    fields
}

fn need_open_comment(target: &PublishTargetConfig) -> String {
    if target.wechat.comment_mode.trim() == "closed" {
        "0"
    } else {
        "1"
    }
    .to_string()
}

fn auto_elect_comment(target: &PublishTargetConfig) -> String {
    if target.wechat.comment_mode.trim() == "auto_selected_open" {
        "1"
    } else {
        "0"
    }
    .to_string()
}

async fn create_wechat_cover_state_via_api<R: BrowserRuntime>(
    runtime: &R,
    origin: &str,
    cover: &WechatCoverRequest,
    plan: &WechatPublishPlan,
) -> Result<WechatCoverState> {
    let image_url = match find_first_wechat_body_image_url(&plan.body_html) {
        Some(url) => url,
        None => {
            return Err(PostpubError::Validation(
                "wechat first_image cover strategy requires an image in article body".to_string(),
            ))
        }
    };

    if !image_url.contains("mmbiz.qpic.cn") {
        return Err(PostpubError::Validation(format!(
            "wechat web api cover from body currently requires a wechat CDN image, got: {image_url}"
        )));
    }

    let mut fields = vec![
        ("imgurl".to_string(), image_url.clone()),
        ("size_count".to_string(), "2".to_string()),
    ];
    fields.extend(crop_fields(
        "size0",
        cover_ratio_crop_percent(cover.size.width, cover.size.height),
    ));
    fields.extend(crop_fields("size1", (0.0, 0.0, 1.0, 1.0)));

    let response = wechat_fetch_form(
        runtime,
        &format!("{origin}/cgi-bin/cropimage?action=crop_multi"),
        &fields,
    )
    .await?;
    let parsed: WechatCropMultiResponse = serde_json::from_str(&response)?;
    ensure_wechat_base_response_ok(parsed.base_resp.as_ref(), &response)?;
    if parsed.result.len() < 2 {
        return Err(PostpubError::External(format!(
            "wechat crop_multi returned {} result(s), expected 2: {response}",
            parsed.result.len()
        )));
    }

    let first = &parsed.result[0];
    let second = &parsed.result[1];
    let first_file_id = value_to_string(&first.file_id).unwrap_or_default();
    let second_file_id = value_to_string(&second.file_id).unwrap_or_default();
    let crop_list = serde_json::json!({
        "crop_list": [
            {
                "ratio": "2.35_1",
                "x1": 0,
                "y1": 0,
                "x2": 0,
                "y2": 0,
                "file_id": first_file_id
            },
            {
                "ratio": "1_1",
                "x1": 0,
                "y1": 0,
                "x2": 0,
                "y2": 0,
                "file_id": second_file_id
            }
        ],
        "crop_list_percent": [
            {
                "ratio": "2.35_1",
                "x1": 0,
                "y1": 0,
                "x2": 1,
                "y2": 1
            },
            {
                "ratio": "1_1",
                "x1": 0,
                "y1": 0,
                "x2": 1,
                "y2": 1
            }
        ]
    })
    .to_string();

    Ok(WechatCoverState {
        cdn_url: first.cdnurl.clone(),
        cdn_235_1_url: first.cdnurl.clone(),
        cdn_1_1_url: second.cdnurl.clone(),
        original_url: image_url,
        crop_list,
        fileid: first_file_id,
    })
}

async fn prepublish_wechat_draft_via_api<R: BrowserRuntime>(
    runtime: &R,
    origin: &str,
    token: &str,
    draft: &WechatDraftState,
) -> Result<()> {
    let page_url = format!(
        "{origin}/cgi-bin/masssendpage?f=json&preview_appmsgid={}&token={token}&lang=zh_CN&ajax=1&random={}",
        draft.appmsgid,
        Utc::now().timestamp_millis()
    );
    let page_response = wechat_fetch_get(runtime, &page_url).await?;
    let page: WechatMassSendPageResponse = serde_json::from_str(&page_response)?;
    ensure_wechat_base_response_ok(page.base_resp.as_ref(), &page_response)?;
    if page.mass_send_left == Some(0) {
        return Err(PostpubError::Conflict(
            "wechat publish quota is exhausted for this account".to_string(),
        ));
    }
    if page.operation_seq.trim().is_empty() {
        return Err(PostpubError::External(format!(
            "wechat masssendpage did not return operation_seq: {page_response}"
        )));
    }
    if !page.strategy_info.trim().is_empty() {
        return Err(PostpubError::Conflict(format!(
            "wechat publish requires account safety verification before final submit: {}",
            page.strategy_info
        )));
    }
    if !page.scene_replace.trim().is_empty() {
        return Err(PostpubError::Conflict(format!(
            "wechat publish requires scene replacement confirmation before final submit: {}",
            page.scene_replace
        )));
    }

    Err(PostpubError::Validation(
        "wechat final publish submit is intentionally disabled until a real account verification run confirms masssend parameters; draft has been saved successfully".to_string(),
    ))
}

fn parse_wechat_draft_response(
    response: WechatDraftApiResponse,
    plan: &WechatPublishPlan,
    previous: Option<&WechatDraftState>,
) -> Result<WechatDraftState> {
    let raw = serde_json::to_string(&response)?;
    ensure_wechat_base_response_ok(response.base_resp.as_ref(), &raw)?;
    if let Some(ret) = response.ret.as_ref().and_then(value_to_string) {
        if ret != "0" {
            return Err(PostpubError::External(format!(
                "wechat operate_appmsg returned ret={ret}: {raw}"
            )));
        }
    }

    let appmsgid = response
        .app_msg_id
        .as_ref()
        .or(response.app_msg_id_alt.as_ref())
        .and_then(value_to_string)
        .or_else(|| previous.map(|state| state.appmsgid.clone()))
        .ok_or_else(|| {
            PostpubError::External(format!(
                "wechat operate_appmsg response missing appMsgId: {raw}"
            ))
        })?;
    let data_seq = response
        .data_seq
        .as_ref()
        .and_then(value_to_string)
        .or_else(|| previous.map(|state| state.data_seq.clone()))
        .ok_or_else(|| {
            PostpubError::External(format!(
                "wechat operate_appmsg response missing data_seq: {raw}"
            ))
        })?;
    let content_html = response
        .filter_content_html
        .as_ref()
        .and_then(|items| items.first())
        .map(|item| item.content.clone())
        .filter(|content| !content.trim().is_empty())
        .or_else(|| previous.map(|state| state.content_html.clone()))
        .unwrap_or_else(|| plan.body_html.clone());

    Ok(WechatDraftState {
        appmsgid,
        data_seq,
        content_html,
        cover: previous.and_then(|state| state.cover.clone()),
    })
}

fn ensure_wechat_base_response_ok(base_resp: Option<&WechatBaseResp>, raw: &str) -> Result<()> {
    let Some(base_resp) = base_resp else {
        return Ok(());
    };
    let ret = base_resp.ret.as_ref().and_then(value_to_string);
    if ret.as_deref().is_some_and(|value| value != "0") {
        let err_msg = base_resp.err_msg.clone().unwrap_or_default();
        return Err(PostpubError::External(format!(
            "wechat api returned base_resp ret={} err_msg={}: {raw}",
            ret.unwrap_or_default(),
            err_msg
        )));
    }
    Ok(())
}

async fn wechat_fetch_get<R: BrowserRuntime>(runtime: &R, url: &str) -> Result<String> {
    let url_json = serde_json::to_string(url)?;
    let script = format!(
        r#"
(async () => {{
  const response = await fetch({url_json}, {{
    credentials: "include",
    headers: {{ "X-Requested-With": "XMLHttpRequest" }}
  }});
  const text = await response.text();
  if (!response.ok) {{
    throw new Error(`HTTP ${{response.status}} ${{response.statusText}}: ${{text}}`);
  }}
  return text;
}})()
"#
    );
    runtime.evaluate(&script).await
}

async fn wechat_fetch_form<R: BrowserRuntime>(
    runtime: &R,
    url: &str,
    fields: &[(String, String)],
) -> Result<String> {
    let url_json = serde_json::to_string(url)?;
    let fields_json = serde_json::to_string(fields)?;
    let script = format!(
        r#"
(async () => {{
  const fields = {fields_json};
  const body = new URLSearchParams();
  for (const [key, value] of fields) {{
    body.append(key, value ?? "");
  }}
  const response = await fetch({url_json}, {{
    method: "POST",
    credentials: "include",
    headers: {{
      "Content-Type": "application/x-www-form-urlencoded; charset=UTF-8",
      "X-Requested-With": "XMLHttpRequest"
    }},
    body
  }});
  const text = await response.text();
  if (!response.ok) {{
    throw new Error(`HTTP ${{response.status}} ${{response.statusText}}: ${{text}}`);
  }}
  return text;
}})()
"#
    );
    runtime.evaluate(&script).await
}

fn build_wechat_digest(html: &str) -> String {
    let tags = Regex::new(r"<[^>]+>").expect("html tag regex");
    tags.replace_all(html, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect()
}

fn find_first_wechat_body_image_url(html: &str) -> Option<String> {
    let regex = Regex::new(r#"(?is)<img\b[^>]*\bsrc\s*=\s*["']([^"']+)["']"#).ok()?;
    regex
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|match_| match_.as_str().trim().to_string())
        .filter(|value| !value.is_empty())
}

fn cover_ratio_crop_percent(width: u32, height: u32) -> (f64, f64, f64, f64) {
    let ratio = width as f64 / height as f64;
    if ratio > 1.0 {
        (0.0, 0.0, 1.0, 1.0 / ratio)
    } else {
        (0.0, 0.0, ratio, 1.0)
    }
}

fn crop_fields(prefix: &str, crop: (f64, f64, f64, f64)) -> Vec<(String, String)> {
    vec![
        (format!("{prefix}_x1"), format_crop_percent(crop.0)),
        (format!("{prefix}_y1"), format_crop_percent(crop.1)),
        (format!("{prefix}_x2"), format_crop_percent(crop.2)),
        (format!("{prefix}_y2"), format_crop_percent(crop.3)),
    ]
}

fn format_crop_percent(value: f64) -> String {
    let value = value.clamp(0.0, 1.0);
    if value == 0.0 || value == 1.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.6}")
    }
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
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
    open_wechat_url(runtime, &editor_url, reporter, "wechat.editor").await?;
    reporter.report("wechat.editor.ready", "direct editor page is ready");
    Ok(())
}

async fn open_wechat_dashboard<R: BrowserRuntime>(
    runtime: &R,
    url: &str,
    reporter: &PublishProgressReporter,
) -> Result<()> {
    open_wechat_url(runtime, url, reporter, "wechat.browser").await?;
    reporter.report("wechat.browser.opened", "wechat dashboard opened");
    reporter.report(
        "wechat.browser.ready",
        "wechat dashboard navigation finished after domcontentloaded",
    );

    Ok(())
}

fn should_continue_after_open_timeout(error: &PostpubError) -> bool {
    matches!(
        error,
        PostpubError::External(message)
            if message.contains("agent-browser command timed out after")
                && message.contains("open ")
    )
}

async fn open_url_with_domcontentloaded<R: BrowserRuntime>(
    runtime: &R,
    url: &str,
    timeout_ms: u64,
) -> Result<()> {
    runtime
        .open_with_wait_until_and_timeout_ms(url, "domcontentloaded", timeout_ms)
        .await
}

async fn open_wechat_url<R: BrowserRuntime>(
    runtime: &R,
    url: &str,
    reporter: &PublishProgressReporter,
    stage_prefix: &str,
) -> Result<()> {
    if let Err(error) =
        open_url_with_domcontentloaded(runtime, url, WECHAT_NAVIGATION_DOM_READY_TIMEOUT_MS).await
    {
        if should_continue_after_open_timeout(&error) {
            reporter.report(
                format!("{stage_prefix}.open_timeout"),
                format!("wechat navigation timed out, continuing with current page state: {error}"),
            );
            return Ok(());
        }

        return Err(error);
    }

    Ok(())
}

async fn ensure_wechat_login_token<R: BrowserRuntime>(
    runtime: &R,
    target: &PublishTargetConfig,
    reporter: &PublishProgressReporter,
) -> Result<String> {
    reporter.report("wechat.auth", "checking wechat login state");
    let current_url = runtime.get_url().await?;
    reporter.report("wechat.auth.url", format!("current page {current_url}"));

    if let Some(token) = extract_query_value(&current_url, "token") {
        return Ok(token);
    }

    let body_text = runtime.get_text("body").await?;
    if looks_like_wechat_login_page(&current_url, &body_text) {
        reporter.report(
            "wechat.auth.required",
            "未检测到微信公众号登录状态，请在内置浏览器中使用微信扫码登录。",
        );
        reporter.report(
            "wechat.auth.waiting",
            "正在等待扫码登录成功，系统会每 1 秒自动检查一次登录状态。",
        );
        return wait_for_wechat_login_token(
            runtime,
            WECHAT_LOGIN_WAIT_TIMEOUT,
            WECHAT_LOGIN_POLL_INTERVAL,
            reporter,
        )
        .await;
    }

    Err(PostpubError::External(format!(
        "failed to determine wechat token from current url for target '{}': {current_url}",
        target.id
    )))
}

async fn wait_for_wechat_login_token<R: BrowserRuntime>(
    runtime: &R,
    timeout: Duration,
    poll_interval: Duration,
    reporter: &PublishProgressReporter,
) -> Result<String> {
    let deadline = Instant::now() + timeout;
    let mut last_url = String::new();

    loop {
        let current_url = runtime.get_url().await?;
        if !current_url.trim().is_empty() {
            last_url = current_url.clone();
        }

        if let Some(token) = extract_query_value(&current_url, "token") {
            reporter.report(
                "wechat.auth.token_detected",
                "检测到微信公众号登录成功，继续执行发布流程。",
            );
            return Ok(token);
        }

        if Instant::now() >= deadline {
            let mut detail =
                "等待微信公众号扫码登录超时，请在内置浏览器中完成扫码并在手机上确认登录后重试。"
                    .to_string();
            if !last_url.is_empty() {
                detail.push_str(&format!(" 最后页面地址：{last_url}"));
            }
            return Err(PostpubError::External(detail));
        }

        runtime.wait_ms(poll_interval.as_millis() as u64).await?;
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

async fn wait_until_true_with_context<R: BrowserRuntime>(
    runtime: &R,
    script: &str,
    timeout: Duration,
    context: &str,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_url = String::new();
    loop {
        let current_url = runtime.get_url().await?;
        if !current_url.trim().is_empty() {
            last_url = current_url;
        }

        if evaluate_bool(runtime, script).await? {
            return Ok(());
        }

        if Instant::now() >= deadline {
            let detail = if last_url.is_empty() {
                format!("timed out waiting for {context}")
            } else {
                format!("timed out waiting for {context} (last url: {last_url})")
            };
            return Err(PostpubError::External(detail));
        }

        sleep(Duration::from_millis(500)).await;
    }
}

async fn evaluate_bool<R: BrowserRuntime>(runtime: &R, script: &str) -> Result<bool> {
    Ok(runtime.evaluate(script).await?.trim() == "true")
}

async fn wait_for_wechat_editor_fields<R: BrowserRuntime>(
    runtime: &R,
    timeout: Duration,
) -> Result<()> {
    wait_until_true_with_context(
        runtime,
        r##"(() => {
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
            return isVisible(document.querySelector(".js_title"))
                && isVisible(document.querySelector(".ProseMirror"))
                && isVisible(document.querySelector("#js_cover_area"));
        })()"##,
        timeout,
        "wechat editor fields",
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
        open_errors: Arc<Mutex<VecDeque<String>>>,
        texts: Arc<Mutex<VecDeque<String>>>,
        urls: Arc<Mutex<VecDeque<String>>>,
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
            if let Some(error) = self.open_errors.lock().unwrap().pop_front() {
                return Err(PostpubError::External(error));
            }
            Ok(())
        }

        async fn open_with_wait_until_and_timeout_ms(
            &self,
            url: &str,
            wait_until: &str,
            timeout_ms: u64,
        ) -> Result<()> {
            self.calls.lock().unwrap().push(format!(
                "open_wait_until_timeout:{url}:{wait_until}:{timeout_ms}"
            ));
            if let Some(error) = self.open_errors.lock().unwrap().pop_front() {
                return Err(PostpubError::External(error));
            }
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
            self.calls.lock().unwrap().push("get_url".to_string());
            Ok(self.urls.lock().unwrap().pop_front().unwrap_or_else(|| {
                "https://mp.weixin.qq.com/cgi-bin/home?t=home/index&lang=zh_CN&token=123"
                    .to_string()
            }))
        }

        async fn get_text(&self, _selector: &str) -> Result<String> {
            self.calls.lock().unwrap().push("get_text:body".to_string());
            Ok(self.texts.lock().unwrap().pop_front().unwrap_or_default())
        }

        async fn evaluate(&self, _script: &str) -> Result<String> {
            self.calls.lock().unwrap().push("evaluate".to_string());
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
    fn article_cover_strategy_falls_back_to_platform_ai_without_local_cover() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        context.bootstrap().expect("bootstrap");

        let article = sample_article("demo.md");
        let target = PublishTargetConfig::default();

        let cover = resolve_wechat_cover_request(
            &context,
            &target,
            &article,
            "<section><p>Body</p></section>",
        )
        .expect("cover request");

        assert!(matches!(cover.plan, WechatCoverPlan::PlatformAi));
    }

    #[test]
    fn custom_path_strategy_falls_back_to_platform_ai_when_file_is_missing() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        context.bootstrap().expect("bootstrap");

        let cover = resolve_wechat_cover_request(
            &context,
            &sample_target(),
            &sample_article("demo.md"),
            "<section><p>Body</p></section>",
        )
        .expect("cover request");

        assert!(matches!(cover.plan, WechatCoverPlan::PlatformAi));
    }

    #[test]
    fn article_cover_strategy_falls_back_to_first_body_image_when_available() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        context.bootstrap().expect("bootstrap");

        let cover = resolve_wechat_cover_request(
            &context,
            &PublishTargetConfig::default(),
            &sample_article("demo.md"),
            r#"<section><img src="https://example.com/body-cover.png" /></section>"#,
        )
        .expect("cover request");

        assert!(matches!(cover.plan, WechatCoverPlan::BodyFirstImage));
    }

    #[test]
    fn first_image_strategy_falls_back_to_platform_ai_when_body_has_no_image() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        context.bootstrap().expect("bootstrap");

        let mut target = PublishTargetConfig::default();
        target.wechat.cover_strategy = "first_image".to_string();

        let cover = resolve_wechat_cover_request(
            &context,
            &target,
            &sample_article("demo.md"),
            "<section><p>Body</p></section>",
        )
        .expect("cover request");

        assert!(matches!(cover.plan, WechatCoverPlan::PlatformAi));
    }

    #[test]
    fn rejects_invalid_wechat_cover_size() {
        let mut target = sample_target();
        target.wechat.cover_width = 0;

        let result = resolve_wechat_cover_size(&target);

        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("invalid wechat cover size"));
    }

    #[test]
    fn builds_digest_from_plain_text_content() {
        let digest = build_wechat_digest("<section><h1>Title</h1><p>Hello world</p></section>");

        assert_eq!(digest, "Title Hello world");
    }

    #[test]
    fn parses_wechat_create_draft_response() {
        let response = WechatDraftApiResponse {
            base_resp: Some(WechatBaseResp {
                ret: Some(Value::from(0)),
                err_msg: Some(String::new()),
            }),
            ret: Some(Value::from("0")),
            app_msg_id: Some(Value::from(100000876)),
            app_msg_id_alt: None,
            data_seq: Some(Value::from("4492886931013566466")),
            filter_content_html: Some(vec![WechatFilteredContent {
                content: "<p>filtered</p>".to_string(),
            }]),
        };
        let plan = WechatPublishPlan {
            title: "Demo".to_string(),
            body_html: "<p>raw</p>".to_string(),
            digest: "raw".to_string(),
            format: "html".to_string(),
            cover: WechatCoverRequest {
                plan: WechatCoverPlan::BodyFirstImage,
                size: WechatCoverSize {
                    width: 900,
                    height: 383,
                },
            },
        };

        let parsed = parse_wechat_draft_response(response, &plan, None).expect("parse response");

        assert_eq!(parsed.appmsgid, "100000876");
        assert_eq!(parsed.data_seq, "4492886931013566466");
        assert_eq!(parsed.content_html, "<p>filtered</p>");
    }

    #[test]
    fn update_fields_include_cover_payload() {
        let target = PublishTargetConfig::default();
        let plan = WechatPublishPlan {
            title: "Demo".to_string(),
            body_html: "<p>body</p>".to_string(),
            digest: "body".to_string(),
            format: "html".to_string(),
            cover: WechatCoverRequest {
                plan: WechatCoverPlan::BodyFirstImage,
                size: WechatCoverSize {
                    width: 900,
                    height: 383,
                },
            },
        };
        let draft = WechatDraftState {
            appmsgid: "100000876".to_string(),
            data_seq: "seq".to_string(),
            content_html: "<p>filtered</p>".to_string(),
            cover: Some(WechatCoverState {
                cdn_url: "https://mmbiz.qpic.cn/main".to_string(),
                cdn_235_1_url: "https://mmbiz.qpic.cn/235".to_string(),
                cdn_1_1_url: "https://mmbiz.qpic.cn/1".to_string(),
                original_url: "https://mmbiz.qpic.cn/original".to_string(),
                crop_list: "{\"crop_list\":[]}".to_string(),
                fileid: "100000877".to_string(),
            }),
        };

        let fields = build_wechat_appmsg_fields(&target, &plan, Some(&draft));

        assert!(fields.contains(&("AppMsgId".to_string(), "100000876".to_string())));
        assert!(fields.contains(&(
            "cdn_235_1_url0".to_string(),
            "https://mmbiz.qpic.cn/235".to_string()
        )));
        assert!(fields.contains(&("fileid0".to_string(), "100000877".to_string())));
    }

    #[tokio::test]
    async fn open_wechat_dashboard_uses_domcontentloaded_open_timeout() {
        let runtime = MockRuntime::default();
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
            calls.iter().any(|entry| {
                entry == "open_wait_until_timeout:https://mp.weixin.qq.com:domcontentloaded:3000"
            }),
            "expected dashboard open to use domcontentloaded wait, got: {calls:?}"
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
    async fn open_wechat_dashboard_continues_after_open_timeout() {
        let runtime = MockRuntime::default();
        runtime.open_errors.lock().unwrap().push_back(
            "agent-browser command timed out after 3s: open https://mp.weixin.qq.com --wait-until domcontentloaded".to_string(),
        );
        let reporter = PublishProgressReporter::new(|_, _| {});

        open_wechat_dashboard(&runtime, "https://mp.weixin.qq.com", &reporter)
            .await
            .expect("wechat dashboard should continue after open timeout");
    }

    #[tokio::test]
    async fn open_wechat_dashboard_still_propagates_non_timeout_open_error() {
        let runtime = MockRuntime::default();
        runtime
            .open_errors
            .lock()
            .unwrap()
            .push_back("navigation failed".to_string());
        let reporter = PublishProgressReporter::new(|_, _| {});

        let error = open_wechat_dashboard(&runtime, "https://mp.weixin.qq.com", &reporter)
            .await
            .expect_err("non-timeout open error should still be returned");

        assert!(error.to_string().contains("navigation failed"));
    }

    #[tokio::test]
    async fn open_wechat_dashboard_skips_interactive_wait() {
        let runtime = MockRuntime::default();
        let reporter = PublishProgressReporter::new(|_, _| {});

        open_wechat_dashboard(&runtime, "https://mp.weixin.qq.com", &reporter)
            .await
            .expect("wechat dashboard should finish after domcontentloaded");

        let calls = runtime.calls.lock().unwrap().clone();
        assert!(
            !calls.iter().any(|entry| entry == "evaluate"),
            "wechat dashboard open should not wait for interactive dashboard, got: {calls:?}"
        );
    }

    #[test]
    fn detects_wechat_login_page_from_qr_login_copy() {
        let body = "登录 使用帐号登录 微信扫一扫，选择该微信下的公众号平台账号登录";

        assert!(looks_like_wechat_login_page(
            "https://mp.weixin.qq.com/",
            body
        ));
    }

    #[tokio::test]
    async fn ensure_wechat_login_token_uses_existing_token_when_available() {
        let runtime = MockRuntime::default();
        runtime.urls.lock().unwrap().push_back(
            "https://mp.weixin.qq.com/cgi-bin/home?t=home/index&lang=zh_CN&token=456".to_string(),
        );
        let reporter = PublishProgressReporter::new(|_, _| {});

        let token = ensure_wechat_login_token(&runtime, &sample_target(), &reporter)
            .await
            .expect("should reuse existing token");

        assert_eq!(token, "456");
    }

    #[tokio::test]
    async fn ensure_wechat_login_token_waits_for_qr_login() {
        let runtime = MockRuntime::default();
        runtime.urls.lock().unwrap().extend([
            "https://mp.weixin.qq.com/".to_string(),
            "https://mp.weixin.qq.com/".to_string(),
            "https://mp.weixin.qq.com/cgi-bin/home?t=home/index&lang=zh_CN&token=789".to_string(),
        ]);
        runtime
            .texts
            .lock()
            .unwrap()
            .push_back("登录 使用帐号登录 微信扫一扫".to_string());
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured_events = events.clone();
        let reporter = PublishProgressReporter::new(move |stage, message| {
            captured_events.lock().unwrap().push((stage, message));
        });

        let token = ensure_wechat_login_token(&runtime, &sample_target(), &reporter)
            .await
            .expect("should wait until token appears");

        assert_eq!(token, "789");
        let recorded = events.lock().unwrap().clone();
        assert!(
            recorded
                .iter()
                .any(|(stage, _)| stage == "wechat.auth.required"),
            "expected login prompt event, got: {recorded:?}"
        );
        assert!(
            recorded
                .iter()
                .any(|(stage, _)| stage == "wechat.auth.waiting"),
            "expected login waiting event, got: {recorded:?}"
        );
        assert!(
            recorded
                .iter()
                .any(|(stage, _)| stage == "wechat.auth.token_detected"),
            "expected token detected event, got: {recorded:?}"
        );
    }

    #[tokio::test]
    async fn wait_for_wechat_login_token_times_out_without_token() {
        let runtime = MockRuntime::default();
        runtime.urls.lock().unwrap().extend([
            "https://mp.weixin.qq.com/".to_string(),
            "https://mp.weixin.qq.com/".to_string(),
        ]);
        let reporter = PublishProgressReporter::new(|_, _| {});

        let error = wait_for_wechat_login_token(
            &runtime,
            Duration::from_millis(0),
            Duration::from_secs(1),
            &reporter,
        )
        .await
        .expect_err("should time out without token");

        let message = error.to_string();
        assert!(message.contains("扫码登录超时"));
        assert!(message.contains("https://mp.weixin.qq.com/"));
    }

    #[tokio::test]
    async fn wait_until_true_with_context_reports_context_and_last_url() {
        let runtime = MockRuntime::default();
        runtime.urls.lock().unwrap().push_back(
            "https://mp.weixin.qq.com/cgi-bin/appmsg?t=media/appmsg_edit_v2".to_string(),
        );
        runtime
            .eval_results
            .lock()
            .unwrap()
            .push_back("false".to_string());

        let error = wait_until_true_with_context(
            &runtime,
            "(() => false)()",
            Duration::from_millis(0),
            "wechat editor fields",
        )
        .await
        .expect_err("wait should time out");

        let message = error.to_string();
        assert!(message.contains("wechat editor fields"));
        assert!(message.contains("appmsg_edit_v2"));
    }

    #[tokio::test]
    async fn check_login_status_reports_valid_when_token_exists() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        let publisher = WechatPublisher::new(context);
        let runtime = MockRuntime::default();
        runtime.urls.lock().unwrap().push_back(
            "https://mp.weixin.qq.com/cgi-bin/home?t=home/index&lang=zh_CN&token=456".to_string(),
        );

        let status = publisher
            .check_login_status_with_runtime(&runtime, &sample_target())
            .await
            .expect("login status should be valid");

        assert!(status.valid);
        assert!(!status.needs_login);
        assert_eq!(
            status.current_url.as_deref(),
            Some("https://mp.weixin.qq.com/cgi-bin/home?t=home/index&lang=zh_CN&token=456")
        );
        let calls = runtime.calls.lock().unwrap().clone();
        assert!(
            !calls.iter().any(|entry| entry == "evaluate"),
            "login status check should not evaluate dashboard readiness, got: {calls:?}"
        );
    }

    #[tokio::test]
    async fn check_login_status_reports_login_required_for_qr_page() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        let publisher = WechatPublisher::new(context);
        let runtime = MockRuntime::default();
        runtime
            .urls
            .lock()
            .unwrap()
            .push_back("https://mp.weixin.qq.com/".to_string());
        runtime.texts.lock().unwrap().push_back(
            "登录 使用帐号登录 微信扫一扫，选择该微信下的公众号平台账号登录".to_string(),
        );

        let status = publisher
            .check_login_status_with_runtime(&runtime, &sample_target())
            .await
            .expect("login status should be returned");

        assert!(!status.valid);
        assert!(status.needs_login);
        assert!(status
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("重新扫码登录"));
        let calls = runtime.calls.lock().unwrap().clone();
        assert!(
            !calls.iter().any(|entry| entry == "evaluate"),
            "login status check should not wait for interactive dashboard, got: {calls:?}"
        );
    }

    #[tokio::test]
    async fn check_login_status_reports_unknown_invalid_state_without_login_prompt() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        let publisher = WechatPublisher::new(context);
        let runtime = MockRuntime::default();
        runtime
            .urls
            .lock()
            .unwrap()
            .push_back("https://mp.weixin.qq.com/cgi-bin/home?t=home/index&lang=zh_CN".to_string());
        runtime
            .texts
            .lock()
            .unwrap()
            .push_back("内容管理 草稿箱 数据概况".to_string());

        let status = publisher
            .check_login_status_with_runtime(&runtime, &sample_target())
            .await
            .expect("login status should be returned");

        assert!(!status.valid);
        assert!(!status.needs_login);
        assert!(status
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("无法确认登录状态"));
        let calls = runtime.calls.lock().unwrap().clone();
        assert!(
            !calls.iter().any(|entry| entry == "evaluate"),
            "login status check should not wait for interactive dashboard, got: {calls:?}"
        );
    }

    #[tokio::test]
    async fn check_login_status_continues_after_open_timeout_for_qr_page() {
        let temp = tempdir().expect("temp dir");
        let context = AppContext::from_root("postpub-core", "0.1.0", temp.path());
        let publisher = WechatPublisher::new(context);
        let runtime = MockRuntime::default();
        runtime.open_errors.lock().unwrap().push_back(
            "agent-browser command timed out after 3s: open https://mp.weixin.qq.com --wait-until domcontentloaded --timeout 3000".to_string(),
        );
        runtime
            .urls
            .lock()
            .unwrap()
            .push_back("https://mp.weixin.qq.com/".to_string());
        runtime
            .texts
            .lock()
            .unwrap()
            .push_back("登录 使用帐号登录 微信扫一扫".to_string());

        let status = publisher
            .check_login_status_with_runtime(&runtime, &sample_target())
            .await
            .expect("login status should still be derived from the current page");

        assert!(!status.valid);
        assert!(status.needs_login);
        let calls = runtime.calls.lock().unwrap().clone();
        assert!(
            !calls.iter().any(|entry| entry == "evaluate"),
            "login status check should not wait for interactive dashboard after open timeout, got: {calls:?}"
        );
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
