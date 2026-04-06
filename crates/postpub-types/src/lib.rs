use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data,
            message: None,
        }
    }

    pub fn with_message(data: T, message: impl Into<String>) -> Self {
        Self {
            success: true,
            data,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            success: false,
            error: error.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub service: String,
    pub status: String,
    pub version: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPathsInfo {
    pub app_root: String,
    pub config_dir: String,
    pub articles_dir: String,
    pub templates_dir: String,
    pub images_dir: String,
    pub logs_dir: String,
    pub temp_dir: String,
    pub config_file: String,
    pub aiforge_config_file: String,
    pub ui_config_file: String,
    pub publish_records_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformSource {
    pub name: String,
    pub weight: f32,
    pub enabled: bool,
}

impl Default for PlatformSource {
    fn default() -> Self {
        Self {
            name: "Google News".to_string(),
            weight: 1.0,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostpubConfig {
    pub platforms: Vec<PlatformSource>,
    pub publish_platform: String,
    #[serde(default)]
    pub img_api: ImageApiConfig,
    #[serde(default = "default_publish_targets")]
    pub publish_targets: Vec<PublishTargetConfig>,
    pub use_template: bool,
    pub template_category: String,
    pub template_name: String,
    pub use_compress: bool,
    pub aiforge_search_max_results: usize,
    pub aiforge_search_min_results: usize,
    pub min_article_len: usize,
    pub max_article_len: usize,
    pub auto_publish: bool,
    pub article_format: String,
    pub format_publish: bool,
}

impl Default for PostpubConfig {
    fn default() -> Self {
        Self {
            platforms: vec![PlatformSource::default()],
            publish_platform: "wechat".to_string(),
            img_api: ImageApiConfig::default(),
            publish_targets: default_publish_targets(),
            use_template: true,
            template_category: "general".to_string(),
            template_name: "magazine".to_string(),
            use_compress: false,
            aiforge_search_max_results: 8,
            aiforge_search_min_results: 3,
            min_article_len: 900,
            max_article_len: 2000,
            auto_publish: false,
            article_format: "html".to_string(),
            format_publish: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImageApiProviderConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageModelProvider {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub enabled: bool,
}

impl Default for ImageModelProvider {
    fn default() -> Self {
        Self {
            id: "image-picsum".to_string(),
            name: "Picsum".to_string(),
            provider_type: "picsum".to_string(),
            api_key: String::new(),
            model: String::new(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageApiConfig {
    #[serde(default = "default_image_api_active_provider_id")]
    pub active_provider_id: String,
    #[serde(default = "default_image_model_providers")]
    pub providers: Vec<ImageModelProvider>,
    #[serde(default = "default_image_api_type")]
    pub api_type: String,
    #[serde(default = "default_ali_image_api_provider")]
    pub ali: ImageApiProviderConfig,
    #[serde(default)]
    pub picsum: ImageApiProviderConfig,
}

fn default_image_api_type() -> String {
    "picsum".to_string()
}

fn default_image_api_active_provider_id() -> String {
    "image-picsum".to_string()
}

fn default_ali_image_api_provider() -> ImageApiProviderConfig {
    ImageApiProviderConfig {
        api_key: String::new(),
        model: "wanx2.0-t2i-turbo".to_string(),
    }
}

fn default_image_model_providers() -> Vec<ImageModelProvider> {
    vec![
        ImageModelProvider::default(),
        ImageModelProvider {
            id: "image-ali".to_string(),
            name: "阿里万相".to_string(),
            provider_type: "ali".to_string(),
            api_key: String::new(),
            model: "wanx2.0-t2i-turbo".to_string(),
            enabled: false,
        },
    ]
}

impl Default for ImageApiConfig {
    fn default() -> Self {
        Self {
            active_provider_id: default_image_api_active_provider_id(),
            providers: default_image_model_providers(),
            api_type: default_image_api_type(),
            ali: default_ali_image_api_provider(),
            picsum: ImageApiProviderConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishTargetConfig {
    pub id: String,
    pub name: String,
    pub platform_type: String,
    #[serde(default)]
    pub account_name: String,
    #[serde(default)]
    pub cookies: String,
    #[serde(default)]
    pub publish_url: String,
    #[serde(default)]
    pub enabled: bool,
    pub article_format: String,
    pub template_category: String,
    pub template_name: String,
    pub min_article_len: usize,
    pub max_article_len: usize,
    #[serde(default)]
    pub use_template: bool,
    #[serde(default)]
    pub use_compress: bool,
    #[serde(default)]
    pub auto_publish: bool,
    #[serde(default)]
    pub format_publish: bool,
}

impl Default for PublishTargetConfig {
    fn default() -> Self {
        Self {
            id: "publish-wechat-1".to_string(),
            name: "微信公众号 1".to_string(),
            platform_type: "wechat".to_string(),
            account_name: String::new(),
            cookies: String::new(),
            publish_url: "https://mp.weixin.qq.com".to_string(),
            enabled: true,
            article_format: "html".to_string(),
            template_category: "general".to_string(),
            template_name: "magazine".to_string(),
            min_article_len: 900,
            max_article_len: 2000,
            use_template: true,
            use_compress: false,
            auto_publish: false,
            format_publish: true,
        }
    }
}

fn default_publish_targets() -> Vec<PublishTargetConfig> {
    vec![PublishTargetConfig::default()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchProviderConfig {
    pub provider: String,
    pub max_results: usize,
    pub request_timeout_secs: u64,
    pub locale: String,
}

impl Default for SearchProviderConfig {
    fn default() -> Self {
        Self {
            provider: "google_news_rss".to_string(),
            max_results: 8,
            request_timeout_secs: 15,
            locale: "zh-CN".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherConfig {
    pub user_agent: String,
    pub request_timeout_secs: u64,
    pub max_content_chars: usize,
}

impl Default for FetcherConfig {
    fn default() -> Self {
        Self {
            user_agent: "postpub/0.1".to_string(),
            request_timeout_secs: 15,
            max_content_chars: 8000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiforgeConfig {
    pub default_search_provider: String,
    pub search: SearchProviderConfig,
    pub fetcher: FetcherConfig,
}

impl Default for AiforgeConfig {
    fn default() -> Self {
        Self {
            default_search_provider: "google_news_rss".to_string(),
            search: SearchProviderConfig::default(),
            fetcher: FetcherConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomLlmProvider {
    pub id: String,
    pub name: String,
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    pub protocol_type: String,
    pub max_tokens: usize,
    pub enabled: bool,
}

impl Default for CustomLlmProvider {
    fn default() -> Self {
        Self {
            id: "custom-1".to_string(),
            name: "Custom".to_string(),
            api_key: String::new(),
            api_base: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            protocol_type: "openai".to_string(),
            max_tokens: 8192,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDesignConfig {
    #[serde(default)]
    pub use_original_styles: bool,
    pub container_max_width: u32,
    pub container_margin_h: u32,
    pub container_bg_color: String,
    pub card_border_radius: u32,
    pub card_padding: u32,
    pub card_bg_color: String,
    pub card_box_shadow: String,
    pub typography_font_size: u32,
    pub typography_line_height: f32,
    pub typography_heading_scale: f32,
    pub typography_text_color: String,
    pub typography_heading_color: String,
    pub spacing_section_margin: u32,
    pub spacing_element_margin: u32,
    pub accent_primary_color: String,
    pub accent_secondary_color: String,
    pub accent_highlight_bg: String,
}

impl Default for PageDesignConfig {
    fn default() -> Self {
        Self {
            use_original_styles: false,
            container_max_width: 750,
            container_margin_h: 10,
            container_bg_color: "#f8f9fa".to_string(),
            card_border_radius: 12,
            card_padding: 24,
            card_bg_color: "#ffffff".to_string(),
            card_box_shadow: "0 4px 16px rgba(0,0,0,0.06)".to_string(),
            typography_font_size: 16,
            typography_line_height: 1.6,
            typography_heading_scale: 1.5,
            typography_text_color: "#333333".to_string(),
            typography_heading_color: "#333333".to_string(),
            spacing_section_margin: 24,
            spacing_element_margin: 16,
            accent_primary_color: "#3a7bd5".to_string(),
            accent_secondary_color: "#00b09b".to_string(),
            accent_highlight_bg: "#f0f7ff".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub window_mode: String,
    #[serde(default = "default_design_theme")]
    pub design_theme: String,
    #[serde(default)]
    pub page_design: PageDesignConfig,
    #[serde(default)]
    pub custom_llm_providers: Vec<CustomLlmProvider>,
}

fn default_design_theme() -> String {
    "follow-system".to_string()
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "light".to_string(),
            window_mode: "STANDARD".to_string(),
            design_theme: default_design_theme(),
            page_design: PageDesignConfig::default(),
            custom_llm_providers: vec![CustomLlmProvider::default()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigBundle {
    pub config: PostpubConfig,
    pub aiforge_config: AiforgeConfig,
    pub ui_config: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateCategorySummary {
    pub name: String,
    pub template_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSummary {
    pub name: String,
    pub category: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDocument {
    pub name: String,
    pub category: String,
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTemplateCategoryRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameTemplateCategoryRequest {
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub category: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTemplateContentRequest {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameTemplateRequest {
    pub relative_path: String,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyTemplateRequest {
    pub relative_path: String,
    pub target_category: String,
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveTemplateRequest {
    pub relative_path: String,
    pub target_category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleSummary {
    pub name: String,
    pub title: String,
    pub relative_path: String,
    pub format: String,
    pub size_bytes: u64,
    pub updated_at: DateTime<Utc>,
    pub status: String,
    #[serde(default)]
    pub variant_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleVariantSummary {
    pub target_id: String,
    pub target_name: String,
    pub platform_type: String,
    pub format: String,
    pub size_bytes: u64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleVariantDocument {
    pub summary: ArticleVariantSummary,
    pub content: String,
    pub preview_html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleDocument {
    pub summary: ArticleSummary,
    pub content: String,
    pub preview_html: String,
    #[serde(default)]
    pub variants: Vec<ArticleVariantDocument>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArticleDesign {
    #[serde(default)]
    pub html: String,
    #[serde(default)]
    pub css: String,
    #[serde(default)]
    pub cover: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateArticleContentRequest {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub abstract_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateArticleRequest {
    pub topic: String,
    #[serde(default)]
    pub reference_urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_name: Option<String>,
    #[serde(default = "default_save_output")]
    pub save_output: bool,
}

fn default_save_output() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationOutput {
    pub title: String,
    pub format: String,
    pub content: String,
    pub preview_html: String,
    #[serde(default)]
    pub variants: Vec<ArticleVariantDocument>,
    pub mode: String,
    pub sources: Vec<SearchResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article: Option<ArticleSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GenerationTaskStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationEvent {
    pub task_id: String,
    pub timestamp: DateTime<Utc>,
    pub stage: String,
    pub message: String,
    pub status: GenerationTaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationTaskSummary {
    pub id: String,
    pub request: GenerateArticleRequest,
    pub status: GenerationTaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub events: Vec<GenerationEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<GenerationOutput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
