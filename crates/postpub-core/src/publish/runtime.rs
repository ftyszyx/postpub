use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};

use crate::{PostpubError, Result};
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

    pub(crate) fn session_name(&self) -> &str {
        &self.session_name
    }

    pub(crate) async fn close(&self) -> Result<()> {
        self.run(&["close"]).await.map(|_| ())
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
        command.arg("--session").arg(&self.session_name);
        command.args(args);
        command.kill_on_drop(true);
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

        let output = timeout(Duration::from_secs(45), child.wait_with_output())
            .await
            .map_err(|_| {
                PostpubError::External(format!(
                    "agent-browser command timed out after 45s: {}",
                    args.join(" ")
                ))
            })??;
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

    if let Some(path) = find_bundled_agent_browser_program() {
        return path.into_os_string();
    }

    #[cfg(windows)]
    let candidates = [
        "agent-browser.cmd",
        "agent-browser.exe",
        "agent-browser.bat",
    ];
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

fn find_bundled_agent_browser_program() -> Option<PathBuf> {
    for candidate in bundled_agent_browser_candidates() {
        if candidate.is_file() {
            return candidate.canonicalize().ok().or(Some(candidate));
        }
    }

    None
}

fn bundled_agent_browser_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(current_exe) = env::current_exe() {
        if let Some(directory) = current_exe.parent() {
            for name in bundled_agent_browser_names() {
                candidates.push(directory.join(name));
                candidates.push(directory.join("bin").join(name));
            }
        }
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    for profile in ["debug", "release"] {
        for name in bundled_agent_browser_names() {
            candidates.push(workspace_root.join("target").join(profile).join(name));
        }
    }

    candidates
}

#[cfg(windows)]
fn bundled_agent_browser_names() -> &'static [&'static str] {
    &["postpub-agent-browser.exe", "postpub-agent-browser.cmd"]
}

#[cfg(not(windows))]
fn bundled_agent_browser_names() -> &'static [&'static str] {
    &["postpub-agent-browser"]
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

fn env_flag(name: &str) -> bool {
    matches!(
        env::var(name)
            .ok()
            .as_deref()
            .map(|value| value.trim().to_ascii_lowercase()),
        Some(value) if matches!(value.as_str(), "1" | "true" | "yes" | "on")
    )
}
