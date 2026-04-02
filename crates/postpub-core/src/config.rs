use std::{fs, path::PathBuf};

use postpub_types::{AiforgeConfig, ConfigBundle, PostpubConfig, UiConfig};

use crate::{error::Result, paths::AppPaths};

#[derive(Debug, Clone)]
pub struct ConfigStore {
    paths: AppPaths,
}

impl ConfigStore {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    pub fn ensure_defaults(&self) -> Result<()> {
        self.paths.ensure_directories()?;

        self.write_if_missing(
            self.paths.config_file(),
            serde_yaml::to_string(&PostpubConfig::default())?,
        )?;
        self.write_if_missing(
            self.paths.aiforge_config_file(),
            toml::to_string_pretty(&AiforgeConfig::default())?,
        )?;
        self.write_if_missing(
            self.paths.ui_config_file(),
            serde_json::to_string_pretty(&UiConfig::default())?,
        )?;
        self.write_if_missing(
            self.paths.publish_records_file(),
            serde_json::to_string_pretty(&serde_json::json!({}))?,
        )?;

        Ok(())
    }

    pub fn load_bundle(&self) -> Result<ConfigBundle> {
        self.ensure_defaults()?;

        Ok(ConfigBundle {
            config: self.load_config()?,
            aiforge_config: self.load_aiforge_config()?,
            ui_config: self.load_ui_config()?,
        })
    }

    pub fn load_config(&self) -> Result<PostpubConfig> {
        Ok(serde_yaml::from_str(&fs::read_to_string(
            self.paths.config_file(),
        )?)?)
    }

    pub fn save_config(&self, config: &PostpubConfig) -> Result<()> {
        fs::write(self.paths.config_file(), serde_yaml::to_string(config)?)?;
        Ok(())
    }

    pub fn load_aiforge_config(&self) -> Result<AiforgeConfig> {
        Ok(toml::from_str(&fs::read_to_string(
            self.paths.aiforge_config_file(),
        )?)?)
    }

    pub fn save_aiforge_config(&self, config: &AiforgeConfig) -> Result<()> {
        fs::write(
            self.paths.aiforge_config_file(),
            toml::to_string_pretty(config)?,
        )?;
        Ok(())
    }

    pub fn load_ui_config(&self) -> Result<UiConfig> {
        Ok(serde_json::from_str(&fs::read_to_string(
            self.paths.ui_config_file(),
        )?)?)
    }

    pub fn save_ui_config(&self, config: &UiConfig) -> Result<()> {
        fs::write(
            self.paths.ui_config_file(),
            serde_json::to_string_pretty(config)?,
        )?;
        Ok(())
    }

    fn write_if_missing(&self, path: PathBuf, content: String) -> Result<()> {
        if !path.exists() {
            fs::write(path, content)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::ConfigStore;
    use crate::paths::AppPaths;

    #[test]
    fn bootstraps_default_config_files() {
        let temp = tempdir().expect("temp dir");
        let store = ConfigStore::new(AppPaths::from_root(temp.path().to_path_buf()));

        store.ensure_defaults().expect("bootstrap defaults");
        let bundle = store.load_bundle().expect("load bundle");

        assert_eq!(bundle.config.template_category, "general");
        assert_eq!(
            bundle.aiforge_config.default_search_provider,
            "google_news_rss"
        );
        assert_eq!(bundle.ui_config.theme, "light");
    }
}
