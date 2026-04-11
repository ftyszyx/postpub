use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use postpub_types::AppPathsInfo;

#[derive(Debug, Clone)]
pub struct AppPaths {
    app_root: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Self {
        if let Ok(override_dir) = env::var("POSTPUB_APP_DATA_DIR") {
            return Self::from_root(override_dir);
        }

        if cfg!(debug_assertions) {
            return Self::from_root(Self::workspace_root().join(".postpub"));
        }

        let app_root = dirs::data_local_dir()
            .or_else(dirs::data_dir)
            .unwrap_or_else(|| Self::workspace_root().join(".postpub"))
            .join("postpub");

        Self { app_root }
    }

    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self {
            app_root: root.into(),
        }
    }

    pub fn app_root(&self) -> &Path {
        &self.app_root
    }

    pub fn config_dir(&self) -> PathBuf {
        self.app_root.join("config")
    }

    pub fn articles_dir(&self) -> PathBuf {
        self.app_root.join("output").join("article")
    }

    pub fn templates_dir(&self) -> PathBuf {
        self.app_root.join("templates")
    }

    pub fn images_dir(&self) -> PathBuf {
        self.app_root.join("image")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.app_root.join("logs")
    }

    pub fn temp_dir(&self) -> PathBuf {
        self.app_root.join("temp")
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.app_root.join("runtime")
    }

    pub fn browser_dir(&self) -> PathBuf {
        self.runtime_dir().join("browser")
    }

    pub fn browser_profiles_dir(&self) -> PathBuf {
        self.runtime_dir().join("profiles")
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir().join("config.yaml")
    }

    pub fn aiforge_config_file(&self) -> PathBuf {
        self.config_dir().join("aiforge.toml")
    }

    pub fn ui_config_file(&self) -> PathBuf {
        self.config_dir().join("ui_config.json")
    }

    pub fn publish_records_file(&self) -> PathBuf {
        self.articles_dir().join("publish_records.json")
    }

    pub fn generation_tasks_file(&self) -> PathBuf {
        self.articles_dir().join("generation_tasks.json")
    }

    pub fn publish_tasks_file(&self) -> PathBuf {
        self.articles_dir().join("publish_tasks.json")
    }

    pub fn ensure_directories(&self) -> io::Result<()> {
        for dir in [
            self.app_root.clone(),
            self.config_dir(),
            self.articles_dir(),
            self.templates_dir(),
            self.images_dir(),
            self.logs_dir(),
            self.temp_dir(),
            self.runtime_dir(),
            self.browser_dir(),
            self.browser_profiles_dir(),
        ] {
            fs::create_dir_all(dir)?;
        }

        Ok(())
    }

    pub fn as_info(&self) -> AppPathsInfo {
        AppPathsInfo {
            app_root: self.app_root.display().to_string(),
            config_dir: self.config_dir().display().to_string(),
            articles_dir: self.articles_dir().display().to_string(),
            templates_dir: self.templates_dir().display().to_string(),
            images_dir: self.images_dir().display().to_string(),
            logs_dir: self.logs_dir().display().to_string(),
            temp_dir: self.temp_dir().display().to_string(),
            runtime_dir: self.runtime_dir().display().to_string(),
            browser_dir: self.browser_dir().display().to_string(),
            browser_profiles_dir: self.browser_profiles_dir().display().to_string(),
            config_file: self.config_file().display().to_string(),
            aiforge_config_file: self.aiforge_config_file().display().to_string(),
            ui_config_file: self.ui_config_file().display().to_string(),
            publish_records_file: self.publish_records_file().display().to_string(),
            publish_tasks_file: self.publish_tasks_file().display().to_string(),
            embedded_browser_executable: self
                .embedded_browser_executable()
                .map(|path| path.display().to_string()),
        }
    }

    pub fn embedded_browser_executable(&self) -> Option<PathBuf> {
        self.browser_executable_candidates()
            .into_iter()
            .find(|path| path.is_file())
            .and_then(|path| path.canonicalize().ok().or(Some(path)))
    }

    fn browser_executable_candidates(&self) -> Vec<PathBuf> {
        let browser_dir = self.browser_dir();
        vec![
            browser_dir.join("chrome.exe"),
            browser_dir.join("chromium.exe"),
            browser_dir.join("chrome"),
            browser_dir.join("chromium"),
            browser_dir.join("chrome-win64").join("chrome.exe"),
            browser_dir.join("chrome-win32").join("chrome.exe"),
            browser_dir.join("chrome-linux64").join("chrome"),
            browser_dir.join("chrome-linux").join("chrome"),
            browser_dir.join("chromium").join("chrome"),
            browser_dir
                .join("Google Chrome for Testing.app")
                .join("Contents")
                .join("MacOS")
                .join("Google Chrome for Testing"),
            browser_dir
                .join("Google Chrome.app")
                .join("Contents")
                .join("MacOS")
                .join("Google Chrome"),
            browser_dir
                .join("Chromium.app")
                .join("Contents")
                .join("MacOS")
                .join("Chromium"),
        ]
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::AppPaths;

    #[test]
    fn builds_expected_child_paths_from_root() {
        let paths = AppPaths::from_root(PathBuf::from("D:/example/postpub-data"));

        assert_eq!(
            paths.config_file(),
            PathBuf::from("D:/example/postpub-data/config/config.yaml")
        );
        assert_eq!(
            paths.aiforge_config_file(),
            PathBuf::from("D:/example/postpub-data/config/aiforge.toml")
        );
        assert_eq!(
            paths.ui_config_file(),
            PathBuf::from("D:/example/postpub-data/config/ui_config.json")
        );
        assert_eq!(
            paths.articles_dir(),
            PathBuf::from("D:/example/postpub-data/output/article")
        );
        assert_eq!(
            paths.publish_records_file(),
            PathBuf::from("D:/example/postpub-data/output/article/publish_records.json")
        );
        assert_eq!(
            paths.generation_tasks_file(),
            PathBuf::from("D:/example/postpub-data/output/article/generation_tasks.json")
        );
        assert_eq!(
            paths.publish_tasks_file(),
            PathBuf::from("D:/example/postpub-data/output/article/publish_tasks.json")
        );
        assert_eq!(
            paths.templates_dir(),
            PathBuf::from("D:/example/postpub-data/templates")
        );
        assert_eq!(
            paths.runtime_dir(),
            PathBuf::from("D:/example/postpub-data/runtime")
        );
        assert_eq!(
            paths.browser_dir(),
            PathBuf::from("D:/example/postpub-data/runtime/browser")
        );
        assert_eq!(
            paths.browser_profiles_dir(),
            PathBuf::from("D:/example/postpub-data/runtime/profiles")
        );
    }

    #[test]
    fn discovers_embedded_browser_inside_runtime_browser_dir() {
        let temp = tempdir().expect("temp dir");
        let paths = AppPaths::from_root(temp.path());
        std::fs::create_dir_all(paths.browser_dir().join("chrome-win64")).expect("browser dir");
        std::fs::write(
            paths.browser_dir().join("chrome-win64").join("chrome.exe"),
            b"stub",
        )
        .expect("browser exe");

        let discovered = paths
            .embedded_browser_executable()
            .expect("embedded browser path");
        assert!(discovered.ends_with("chrome.exe"));
    }
}
