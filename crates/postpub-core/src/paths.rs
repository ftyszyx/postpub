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

    pub fn ensure_directories(&self) -> io::Result<()> {
        for dir in [
            self.app_root.clone(),
            self.config_dir(),
            self.articles_dir(),
            self.templates_dir(),
            self.images_dir(),
            self.logs_dir(),
            self.temp_dir(),
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
            config_file: self.config_file().display().to_string(),
            aiforge_config_file: self.aiforge_config_file().display().to_string(),
            ui_config_file: self.ui_config_file().display().to_string(),
            publish_records_file: self.publish_records_file().display().to_string(),
        }
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
            paths.templates_dir(),
            PathBuf::from("D:/example/postpub-data/templates")
        );
    }
}
