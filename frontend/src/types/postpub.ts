export interface HealthStatus {
  service: string;
  status: string;
  version: string;
  timestamp: string;
}

export interface AppPathsInfo {
  app_root: string;
  config_dir: string;
  articles_dir: string;
  templates_dir: string;
  images_dir: string;
  logs_dir: string;
  temp_dir: string;
  runtime_dir: string;
  browser_dir: string;
  browser_profiles_dir: string;
  config_file: string;
  aiforge_config_file: string;
  ui_config_file: string;
  publish_records_file: string;
  publish_tasks_file: string;
  embedded_browser_executable?: string | null;
}

export interface BrowserEnvironmentStatus {
  config_url: string;
  remote_version?: string | null;
  remote_asset_url?: string | null;
  remote_error?: string | null;
  browser_dir: string;
  browser_profiles_dir: string;
  manifest_path: string;
  local_version?: string | null;
  browser_executable?: string | null;
  browser_ready: boolean;
  profile_dir?: string | null;
  profile_exists: boolean;
  profile_entry_count: number;
}

export interface PlatformSource {
  name: string;
  weight: number;
  enabled: boolean;
}

export interface PostpubConfig {
  platforms: PlatformSource[];
  publish_platform: string;
  img_api: ImageApiConfig;
  publish_targets: PublishTargetConfig[];
  use_template: boolean;
  template_category: string;
  template_name: string;
  use_compress: boolean;
  aiforge_search_max_results: number;
  aiforge_search_min_results: number;
  min_article_len: number;
  max_article_len: number;
  auto_publish: boolean;
  article_format: string;
  format_publish: boolean;
}

export interface ImageApiProviderConfig {
  api_key: string;
  model: string;
}

export interface ImageModelProvider {
  id: string;
  name: string;
  provider_type: string;
  api_key: string;
  model: string;
  enabled: boolean;
}

export interface ImageApiConfig {
  active_provider_id: string;
  providers: ImageModelProvider[];
  api_type: string;
  ali: ImageApiProviderConfig;
  picsum: ImageApiProviderConfig;
}

export interface PublishTargetConfig {
  id: string;
  name: string;
  platform_type: string;
  account_name: string;
  cookies: string;
  publish_url: string;
  enabled: boolean;
  article_format: string;
  template_category: string;
  template_name: string;
  min_article_len: number;
  max_article_len: number;
  use_template: boolean;
  use_compress: boolean;
  auto_publish: boolean;
  format_publish: boolean;
  wechat: WechatPublishTargetConfig;
}

export interface WechatPublishTargetConfig {
  cover_strategy: string;
  cover_path: string;
  cover_width: number;
  cover_height: number;
  declare_original: boolean;
  enable_reward: boolean;
  enable_paid: boolean;
  comment_mode: string;
  collection_id: string;
  source_url: string;
  source_label: string;
  platform_recommendation_enabled: boolean;
}

export interface SearchProviderConfig {
  provider: string;
  max_results: number;
  request_timeout_secs: number;
  locale: string;
}

export interface FetcherConfig {
  user_agent: string;
  request_timeout_secs: number;
  max_content_chars: number;
}

export interface AiforgeConfig {
  default_search_provider: string;
  search: SearchProviderConfig;
  fetcher: FetcherConfig;
}

export interface CustomLlmProvider {
  id: string;
  name: string;
  api_key: string;
  api_base: string;
  model: string;
  protocol_type: string;
  max_tokens: number;
  enabled: boolean;
}

export interface PageDesignConfig {
  use_original_styles: boolean;
  container_max_width: number;
  container_margin_h: number;
  container_bg_color: string;
  card_border_radius: number;
  card_padding: number;
  card_bg_color: string;
  card_box_shadow: string;
  typography_font_size: number;
  typography_line_height: number;
  typography_heading_scale: number;
  typography_text_color: string;
  typography_heading_color: string;
  spacing_section_margin: number;
  spacing_element_margin: number;
  accent_primary_color: string;
  accent_secondary_color: string;
  accent_highlight_bg: string;
}

export interface UiConfig {
  theme: string;
  window_mode: string;
  design_theme: string;
  page_design: PageDesignConfig;
  custom_llm_providers: CustomLlmProvider[];
}

export interface ConfigBundle {
  config: PostpubConfig;
  aiforge_config: AiforgeConfig;
  ui_config: UiConfig;
}

export interface TemplateCategorySummary {
  name: string;
  template_count: number;
}

export interface TemplateSummary {
  name: string;
  category: string;
  relative_path: string;
  size_bytes: number;
  updated_at: string;
}

export interface TemplateDocument {
  name: string;
  category: string;
  relative_path: string;
  content: string;
}

export interface ArticleSummary {
  name: string;
  title: string;
  relative_path: string;
  format: string;
  size_bytes: number;
  updated_at: string;
  status: string;
  variant_count: number;
}

export interface ArticleVariantSummary {
  target_id: string;
  target_name: string;
  platform_type: string;
  format: string;
  size_bytes: number;
  updated_at: string;
}

export interface ArticleVariantDocument {
  summary: ArticleVariantSummary;
  content: string;
  preview_html: string;
}

export interface ArticleDocument {
  summary: ArticleSummary;
  content: string;
  preview_html: string;
  variants: ArticleVariantDocument[];
}

export interface ArticleDesign {
  html: string;
  css: string;
  cover: string;
}

export interface SearchResult {
  title: string;
  url: string;
  abstract_text: string;
  published_at?: string;
  content?: string;
}

export interface GenerateArticleRequest {
  topic: string;
  reference_urls: string[];
  template_category?: string;
  template_name?: string;
  save_output: boolean;
}

export interface GenerationOutput {
  title: string;
  format: string;
  content: string;
  preview_html: string;
  variants: ArticleVariantDocument[];
  mode: string;
  sources: SearchResult[];
  article?: ArticleSummary;
}

export type GenerationTaskStatus = "Pending" | "Running" | "Succeeded" | "Failed";

export interface GenerationEvent {
  task_id: string;
  timestamp: string;
  stage: string;
  message: string;
  status: GenerationTaskStatus;
}

export interface GenerationTaskSummary {
  id: string;
  request: GenerateArticleRequest;
  status: GenerationTaskStatus;
  created_at: string;
  updated_at: string;
  events: GenerationEvent[];
  output?: GenerationOutput;
  error?: string;
}

export interface PublishArticleRequest {
  article_relative_path: string;
  target_id: string;
  mode: string;
}

export interface PublishOutput {
  article_relative_path: string;
  article_title: string;
  target_id: string;
  target_name: string;
  platform_type: string;
  mode: string;
  format: string;
  remote_id?: string;
  remote_url?: string;
}

export type PublishTaskStatus = "Pending" | "Running" | "Succeeded" | "Failed";

export interface PublishEvent {
  task_id: string;
  timestamp: string;
  stage: string;
  message: string;
  status: PublishTaskStatus;
}

export interface PublishTaskSummary {
  id: string;
  request: PublishArticleRequest;
  status: PublishTaskStatus;
  created_at: string;
  updated_at: string;
  events: PublishEvent[];
  output?: PublishOutput;
  error?: string;
}
