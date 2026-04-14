use std::{
    env, fs, io,
    path::{Component, Path, PathBuf},
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

        Self::from_root(app_root)
    }

    pub fn from_root(root: impl Into<PathBuf>) -> Self {
        Self {
            app_root: normalize_absolute_path(root.into()),
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
            app_root: display_path(&self.app_root),
            config_dir: display_path(&self.config_dir()),
            articles_dir: display_path(&self.articles_dir()),
            templates_dir: display_path(&self.templates_dir()),
            images_dir: display_path(&self.images_dir()),
            logs_dir: display_path(&self.logs_dir()),
            temp_dir: display_path(&self.temp_dir()),
            runtime_dir: display_path(&self.runtime_dir()),
            browser_dir: display_path(&self.browser_dir()),
            browser_profiles_dir: display_path(&self.browser_profiles_dir()),
            config_file: display_path(&self.config_file()),
            aiforge_config_file: display_path(&self.aiforge_config_file()),
            ui_config_file: display_path(&self.ui_config_file()),
            publish_records_file: display_path(&self.publish_records_file()),
            publish_tasks_file: display_path(&self.publish_tasks_file()),
            embedded_browser_executable: self
                .embedded_browser_executable()
                .map(|path| display_path(&path)),
        }
    }

    pub fn embedded_browser_executable(&self) -> Option<PathBuf> {
        self.browser_executable_candidates()
            .into_iter()
            .find(|path| path.is_file())
            .and_then(|path| {
                path.canonicalize()
                    .ok()
                    .or_else(|| Some(normalize_absolute_path(path)))
            })
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
        normalize_absolute_path(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(".."),
        )
    }
}

pub(crate) fn normalize_absolute_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map(|current_dir| current_dir.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };

    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

pub(crate) fn display_path(path: impl AsRef<Path>) -> String {
    let mut rendered = normalize_absolute_path(path).display().to_string();

    #[cfg(windows)]
    {
        if let Some(stripped) = rendered.strip_prefix(r"\\?\UNC\") {
            rendered = format!(r"\\{stripped}");
        } else if let Some(stripped) = rendered.strip_prefix(r"\\?\") {
            rendered = stripped.to_string();
        }
    }

    rendered
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::{normalize_absolute_path, AppPaths};

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

    #[test]
    fn normalizes_root_with_parent_segments() {
        let temp = tempdir().expect("temp dir");
        let root = temp.path().join("runtime").join("..").join("postpub-data");
        let paths = AppPaths::from_root(&root);

        assert_eq!(paths.app_root(), normalize_absolute_path(root).as_path());
        assert!(
            !paths.app_root().to_string_lossy().contains(".."),
            "normalized app root should not contain parent segments"
        );
    }
}
