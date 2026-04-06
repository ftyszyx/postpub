mod aiforge;
mod articles;
mod config;
mod error;
mod generation;
mod paths;
mod templates;

use chrono::Utc;
use postpub_types::HealthStatus;
use reqwest::Client;

pub use aiforge::AiforgeEngine;
pub use articles::{markdown_to_html, preview_html, ArticleStore};
pub use config::ConfigStore;
pub use error::{PostpubError, Result};
pub use generation::{GenerationProgressReporter, GenerationService};
pub use paths::AppPaths;
pub use templates::TemplateStore;

#[derive(Debug, Clone)]
pub struct AppContext {
    service_name: String,
    version: String,
    paths: AppPaths,
    http_client: Client,
}

impl AppContext {
    pub fn new(service_name: impl Into<String>, version: impl Into<String>) -> Self {
        Self::from_root(
            service_name,
            version,
            AppPaths::discover().app_root().to_path_buf(),
        )
    }

    pub fn from_root(
        service_name: impl Into<String>,
        version: impl Into<String>,
        root: impl Into<std::path::PathBuf>,
    ) -> Self {
        let http_client = Client::builder()
            .build()
            .expect("failed to create reqwest client");

        Self {
            service_name: service_name.into(),
            version: version.into(),
            paths: AppPaths::from_root(root),
            http_client,
        }
    }

    pub fn bootstrap(&self) -> Result<()> {
        self.paths.ensure_directories()?;
        self.config_store().ensure_defaults()?;
        self.template_store().ensure_defaults()?;
        self.article_store().ensure_defaults()?;
        Ok(())
    }

    pub fn health_status(&self) -> HealthStatus {
        HealthStatus {
            service: self.service_name.clone(),
            status: "ok".to_string(),
            version: self.version.clone(),
            timestamp: Utc::now(),
        }
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn http_client(&self) -> &Client {
        &self.http_client
    }

    pub fn config_store(&self) -> ConfigStore {
        ConfigStore::new(self.paths.clone())
    }

    pub fn template_store(&self) -> TemplateStore {
        TemplateStore::new(self.paths.clone())
    }

    pub fn article_store(&self) -> ArticleStore {
        ArticleStore::new(self.paths.clone())
    }

    pub fn generation_service(&self) -> GenerationService {
        GenerationService::new(self.clone())
    }
}
