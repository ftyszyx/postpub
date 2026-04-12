use std::{fs, path::PathBuf};

use postpub_types::{AiforgeConfig, ConfigBundle, PostpubConfig, UiConfig};

use crate::{error::Result, paths::AppPaths, text::repair_mojibake};

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
        let content = fs::read_to_string(self.paths.config_file())?;
        let mut config: PostpubConfig = serde_yaml::from_str(&content)?;

        if repair_postpub_config(&mut config) {
            self.save_config(&config)?;
        }

        Ok(config)
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
        let content = fs::read_to_string(self.paths.ui_config_file())?;
        let mut ui_config: UiConfig = serde_json::from_str(&content)?;

        if repair_ui_config(&mut ui_config) {
            self.save_ui_config(&ui_config)?;
        }

        Ok(ui_config)
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

fn repair_postpub_config(config: &mut PostpubConfig) -> bool {
    let mut changed = false;

    for platform in &mut config.platforms {
        changed |= repair_string(&mut platform.name);
    }

    changed |= repair_string(&mut config.template_category);
    changed |= repair_string(&mut config.template_name);

    for provider in &mut config.img_api.providers {
        changed |= repair_string(&mut provider.name);
    }

    for target in &mut config.publish_targets {
        changed |= repair_string(&mut target.name);
        changed |= repair_string(&mut target.account_name);
        changed |= repair_string(&mut target.template_category);
        changed |= repair_string(&mut target.template_name);
        changed |= repair_string(&mut target.wechat.source_label);
    }

    changed
}

fn repair_ui_config(ui_config: &mut UiConfig) -> bool {
    let mut changed = false;

    for provider in &mut ui_config.custom_llm_providers {
        changed |= repair_string(&mut provider.name);
    }

    changed
}

fn repair_string(value: &mut String) -> bool {
    let Some(repaired) = repair_mojibake(value) else {
        return false;
    };

    *value = repaired;
    true
}

#[cfg(test)]
mod tests {
    use std::fs;

    use postpub_types::PostpubConfig;
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

    #[test]
    fn repairs_mojibake_names_when_loading_config() {
        let temp = tempdir().expect("temp dir");
        let store = ConfigStore::new(AppPaths::from_root(temp.path().to_path_buf()));

        store.ensure_defaults().expect("bootstrap defaults");

        let mut config = PostpubConfig::default();
        config.img_api.providers[1].name = latin1_mojibake("阿里万相");
        config.publish_targets[0].name = latin1_mojibake("微信公众号 1");

        let config_file = AppPaths::from_root(temp.path().to_path_buf()).config_file();
        fs::write(&config_file, serde_yaml::to_string(&config).expect("serialize config"))
            .expect("write config");

        let loaded = store.load_config().expect("load repaired config");
        assert_eq!(loaded.img_api.providers[1].name, "阿里万相");
        assert_eq!(loaded.publish_targets[0].name, "微信公众号 1");

        let persisted = fs::read_to_string(config_file).expect("read repaired file");
        assert!(persisted.contains("阿里万相"));
        assert!(persisted.contains("微信公众号 1"));
    }

    fn latin1_mojibake(value: &str) -> String {
        value
            .as_bytes()
            .iter()
            .map(|byte| char::from(*byte))
            .collect()
    }
}
