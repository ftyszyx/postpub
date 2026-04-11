use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

use chrono::Utc;
use postpub_types::BrowserEnvironmentStatus;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::{AppPaths, PostpubError, Result};

const DEFAULT_BROWSER_CONFIG_URL: &str = "https://www.bytefuse.cn/clonerweibo.json";
const BROWSER_MANIFEST_FILE: &str = "postpub-browser.json";

#[derive(Debug, Clone)]
pub struct BrowserManager {
    paths: AppPaths,
    http_client: Client,
}

impl BrowserManager {
    pub fn new(paths: AppPaths, http_client: Client) -> Self {
        Self { paths, http_client }
    }

    pub async fn ensure_browser_executable(&self) -> Result<PathBuf> {
        self.ensure_browser_executable_with_progress(|_, _| {})
            .await
    }

    pub async fn ensure_browser_executable_with_progress<F>(&self, mut report: F) -> Result<PathBuf>
    where
        F: FnMut(&str, &str),
    {
        if let Ok(value) = std::env::var("POSTPUB_BROWSER_EXECUTABLE") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                let path = PathBuf::from(trimmed);
                if path.is_file() {
                    report("browser.override", trimmed);
                    return Ok(path.canonicalize().ok().unwrap_or(path));
                }

                return Err(PostpubError::NotFound(format!(
                    "configured browser executable does not exist: {trimmed}"
                )));
            }
        }

        self.paths.ensure_directories()?;

        report("browser.config", "fetch remote browser config");
        let remote = self.fetch_remote_config().await?;
        if let Some(path) = self.current_browser_if_matching(&remote)? {
            report(
                "browser.cached",
                &format!("reuse embedded chrome {}", remote.chrome_version),
            );
            return Ok(path);
        }

        report(
            "browser.download",
            &format!(
                "download embedded chrome {} from {}",
                remote.chrome_version, remote.chrome_asset_url
            ),
        );
        let path = self.download_and_install(&remote).await?;
        report(
            "browser.ready",
            &format!("embedded chrome installed at {}", path.display()),
        );
        Ok(path)
    }

    pub async fn status(&self, target_id: Option<&str>) -> Result<BrowserEnvironmentStatus> {
        self.paths.ensure_directories()?;

        let config_url = std::env::var("POSTPUB_BROWSER_CONFIG_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BROWSER_CONFIG_URL.to_string());

        let (remote_version, remote_asset_url, remote_error) =
            match self.fetch_remote_config().await {
                Ok(config) => (
                    Some(config.chrome_version),
                    Some(config.chrome_asset_url),
                    None,
                ),
                Err(error) => (None, None, Some(error.to_string())),
            };

        let manifest_path = self.manifest_path();
        let local_manifest = self.load_local_manifest()?;
        let browser_executable = self
            .paths
            .embedded_browser_executable()
            .map(|path| path.display().to_string());
        let browser_ready = browser_executable.is_some();

        let profile_dir = target_id.map(|target| browser_profile_dir(&self.paths, target));
        let profile_exists = profile_dir
            .as_ref()
            .map(|dir| dir.exists())
            .unwrap_or(false);
        let profile_entry_count = profile_dir
            .as_ref()
            .map(|dir| count_profile_entries(dir))
            .transpose()?
            .unwrap_or(0);

        Ok(BrowserEnvironmentStatus {
            config_url,
            remote_version,
            remote_asset_url,
            remote_error,
            browser_dir: self.paths.browser_dir().display().to_string(),
            browser_profiles_dir: self.paths.browser_profiles_dir().display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            local_version: local_manifest
                .as_ref()
                .map(|manifest| manifest.chrome_version.clone()),
            browser_executable,
            browser_ready,
            profile_dir: profile_dir.map(|dir| dir.display().to_string()),
            profile_exists,
            profile_entry_count,
        })
    }

    pub fn clear_profile(&self, target_id: &str) -> Result<PathBuf> {
        self.paths.ensure_directories()?;
        let profile_dir = browser_profile_dir(&self.paths, target_id);
        if profile_dir.exists() {
            fs::remove_dir_all(&profile_dir)?;
        }
        fs::create_dir_all(&profile_dir)?;
        Ok(profile_dir)
    }

    fn current_browser_if_matching(&self, remote: &RemoteBrowserConfig) -> Result<Option<PathBuf>> {
        let manifest_path = self.manifest_path();
        if !manifest_path.exists() {
            return Ok(None);
        }

        let manifest: EmbeddedBrowserManifest =
            serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
        if manifest.chrome_version.trim() != remote.chrome_version.trim()
            || manifest.chrome_asset_url.trim() != remote.chrome_asset_url.trim()
        {
            return Ok(None);
        }

        let executable = self
            .paths
            .browser_dir()
            .join(&manifest.executable_relative_path);
        if executable.is_file() {
            return Ok(Some(executable.canonicalize().ok().unwrap_or(executable)));
        }

        Ok(None)
    }

    async fn fetch_remote_config(&self) -> Result<RemoteBrowserConfig> {
        let config_url = std::env::var("POSTPUB_BROWSER_CONFIG_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_BROWSER_CONFIG_URL.to_string());

        let response = self.http_client.get(&config_url).send().await?;
        let response = response.error_for_status()?;
        let config: RemoteBrowserConfig = response.json().await?;

        if config.chrome_asset_url.trim().is_empty() || config.chrome_version.trim().is_empty() {
            return Err(PostpubError::External(format!(
                "browser config is missing chrome download info: {config_url}"
            )));
        }

        Ok(config)
    }

    async fn download_and_install(&self, remote: &RemoteBrowserConfig) -> Result<PathBuf> {
        let response = self
            .http_client
            .get(&remote.chrome_asset_url)
            .send()
            .await?;
        let response = response.error_for_status()?;
        let bytes = response.bytes().await?;
        let zip_bytes = bytes.to_vec();

        let install_root = self.paths.browser_dir();
        let extract_root = self
            .paths
            .temp_dir()
            .join(format!("browser-install-{}", Utc::now().timestamp_millis()));

        if extract_root.exists() {
            fs::remove_dir_all(&extract_root)?;
        }
        fs::create_dir_all(&extract_root)?;
        unpack_zip(&zip_bytes, &extract_root)?;

        let executable = find_browser_executable_in_dir(&extract_root).ok_or_else(|| {
            PostpubError::NotFound(
                "downloaded browser archive did not contain a supported browser executable"
                    .to_string(),
            )
        })?;

        let executable_relative_path = executable
            .strip_prefix(&extract_root)
            .map_err(|_| PostpubError::InvalidPath(executable.display().to_string()))?
            .to_path_buf();

        if install_root.exists() {
            fs::remove_dir_all(&install_root)?;
        }
        fs::create_dir_all(
            install_root
                .parent()
                .ok_or_else(|| PostpubError::InvalidPath(install_root.display().to_string()))?,
        )?;
        fs::rename(&extract_root, &install_root)?;

        let installed_executable = install_root.join(&executable_relative_path);
        let manifest = EmbeddedBrowserManifest {
            chrome_version: remote.chrome_version.clone(),
            chrome_asset_url: remote.chrome_asset_url.clone(),
            executable_relative_path: normalize_relative_path(&executable_relative_path),
            synced_at: Utc::now().to_rfc3339(),
        };
        fs::write(
            self.manifest_path(),
            serde_json::to_string_pretty(&manifest)?,
        )?;

        Ok(installed_executable
            .canonicalize()
            .ok()
            .unwrap_or(installed_executable))
    }

    fn manifest_path(&self) -> PathBuf {
        self.paths.browser_dir().join(BROWSER_MANIFEST_FILE)
    }

    fn load_local_manifest(&self) -> Result<Option<EmbeddedBrowserManifest>> {
        let path = self.manifest_path();
        if !path.exists() {
            return Ok(None);
        }

        Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RemoteBrowserConfig {
    chrome_asset_url: String,
    chrome_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmbeddedBrowserManifest {
    chrome_version: String,
    chrome_asset_url: String,
    executable_relative_path: String,
    synced_at: String,
}

pub fn browser_profile_dir(paths: &AppPaths, target_id: &str) -> PathBuf {
    paths
        .browser_profiles_dir()
        .join(sanitize_profile_component(target_id))
}

pub fn sanitize_profile_component(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut last_dash = false;
    for ch in value.chars() {
        let normalized = if ch.is_ascii_alphanumeric() { ch } else { '-' };
        if normalized == '-' {
            if last_dash {
                continue;
            }
            last_dash = true;
        } else {
            last_dash = false;
        }
        output.push(normalized.to_ascii_lowercase());
    }

    let normalized = output.trim_matches('-').to_string();
    if normalized.is_empty() {
        "default".to_string()
    } else {
        normalized
    }
}

fn unpack_zip(bytes: &[u8], target_dir: &Path) -> Result<()> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|error| PostpubError::External(format!("failed to read browser zip: {error}")))?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| {
            PostpubError::External(format!("failed to read browser zip entry: {error}"))
        })?;

        let Some(relative_path) = entry.enclosed_name().map(|path| path.to_path_buf()) else {
            continue;
        };
        let output_path = target_dir.join(relative_path);

        if entry.name().ends_with('/') {
            fs::create_dir_all(&output_path)?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut output = fs::File::create(&output_path)?;
        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer).map_err(|error| {
            PostpubError::External(format!("failed to extract browser zip entry: {error}"))
        })?;
        output.write_all(&buffer)?;
    }

    Ok(())
}

fn find_browser_executable_in_dir(root: &Path) -> Option<PathBuf> {
    let candidates = [
        root.join("chrome.exe"),
        root.join("chromium.exe"),
        root.join("chrome"),
        root.join("chromium"),
        root.join("chrome-win64").join("chrome.exe"),
        root.join("chrome-win32").join("chrome.exe"),
        root.join("chrome-linux64").join("chrome"),
        root.join("chrome-linux").join("chrome"),
        root.join("chromium").join("chrome"),
        root.join("Google Chrome for Testing.app")
            .join("Contents")
            .join("MacOS")
            .join("Google Chrome for Testing"),
        root.join("Google Chrome.app")
            .join("Contents")
            .join("MacOS")
            .join("Google Chrome"),
        root.join("Chromium.app")
            .join("Contents")
            .join("MacOS")
            .join("Chromium"),
    ];

    candidates.into_iter().find(|path| path.is_file())
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn count_profile_entries(path: &Path) -> Result<usize> {
    if !path.exists() {
        return Ok(0);
    }

    Ok(fs::read_dir(path)?.filter_map(|entry| entry.ok()).count())
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    use super::{find_browser_executable_in_dir, unpack_zip};
    use crate::AppContext;

    #[test]
    fn extracts_zip_and_finds_browser_executable() {
        let temp = tempdir().expect("temp dir");
        let mut writer = ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        let options = SimpleFileOptions::default();
        writer
            .add_directory("chrome-win64/", options)
            .expect("add dir");
        writer
            .start_file("chrome-win64/chrome.exe", options)
            .expect("start file");
        writer.write_all(b"stub").expect("write file");

        let cursor = writer.finish().expect("finish zip");
        let bytes = cursor.into_inner();

        unpack_zip(&bytes, temp.path()).expect("unpack");
        let executable =
            find_browser_executable_in_dir(temp.path()).expect("browser executable should exist");
        assert!(executable.ends_with("chrome.exe"));
    }

    #[tokio::test]
    #[ignore = "downloads real browser asset from remote config"]
    async fn syncs_remote_browser_into_real_app_runtime_dir() {
        let context = AppContext::new("postpub-core", "0.1.0");
        context.bootstrap().expect("bootstrap");

        let executable = context
            .browser_manager()
            .ensure_browser_executable_with_progress(|stage, message| {
                println!("{stage}: {message}");
            })
            .await
            .expect("sync browser");

        assert!(executable.is_file());
    }
}
