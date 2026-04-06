import { defineStore } from "pinia";
import { apiGet, apiPut, type ApiResponse } from "../api/client";
import type {
  ConfigBundle,
  CustomLlmProvider,
  ImageApiConfig,
  ImageModelProvider,
  ImageApiProviderConfig,
  PageDesignConfig,
  PostpubConfig,
  PublishTargetConfig,
  UiConfig
} from "../types/postpub";
import { translate } from "../utils/i18n";

const DEFAULT_THEME = "light";
const DEFAULT_WINDOW_MODE = "STANDARD";
const DEFAULT_DESIGN_THEME = "follow-system";

let systemThemeMediaQuery: MediaQueryList | null = null;
let systemThemeListenerBound = false;

const defaultPageDesign = (): PageDesignConfig => ({
  use_original_styles: false,
  container_max_width: 750,
  container_margin_h: 10,
  container_bg_color: "#f8f9fa",
  card_border_radius: 12,
  card_padding: 24,
  card_bg_color: "#ffffff",
  card_box_shadow: "0 4px 16px rgba(0,0,0,0.06)",
  typography_font_size: 16,
  typography_line_height: 1.6,
  typography_heading_scale: 1.5,
  typography_text_color: "#333333",
  typography_heading_color: "#333333",
  spacing_section_margin: 24,
  spacing_element_margin: 16,
  accent_primary_color: "#3a7bd5",
  accent_secondary_color: "#00b09b",
  accent_highlight_bg: "#f0f7ff"
});

const defaultImageApiProvider = (overrides: Partial<ImageApiProviderConfig> = {}): ImageApiProviderConfig => ({
  api_key: "",
  model: "",
  ...overrides
});

const defaultImageModelProvider = (
  providerType: string,
  index = 1,
  overrides: Partial<ImageModelProvider> = {}
): ImageModelProvider => ({
  id: `image-${providerType}-${index}`,
  name: providerType === "ali" ? `阿里万相 ${index}` : `Picsum ${index}`,
  provider_type: providerType,
  api_key: "",
  model: providerType === "ali" ? "wanx2.0-t2i-turbo" : "",
  enabled: providerType === "picsum",
  ...overrides
});

const defaultImageApiConfig = (): ImageApiConfig => ({
  active_provider_id: "image-picsum-1",
  providers: [
    defaultImageModelProvider("picsum", 1, {
      id: "image-picsum-1",
      name: "Picsum",
      enabled: true
    }),
    defaultImageModelProvider("ali", 1, {
      id: "image-ali-1",
      name: "阿里万相",
      enabled: false
    })
  ],
  api_type: "picsum",
  ali: defaultImageApiProvider({ model: "wanx2.0-t2i-turbo" }),
  picsum: defaultImageApiProvider()
});

const defaultPublishTarget = (index = 1, overrides: Partial<PublishTargetConfig> = {}): PublishTargetConfig => ({
  id: `publish-wechat-${index}`,
  name: `微信公众号 ${index}`,
  platform_type: "wechat",
  account_name: "",
  cookies: "",
  publish_url: "https://mp.weixin.qq.com",
  enabled: index === 1,
  article_format: "html",
  template_category: "general",
  template_name: "magazine",
  min_article_len: 900,
  max_article_len: 2000,
  use_template: true,
  use_compress: false,
  auto_publish: false,
  format_publish: true,
  ...overrides
});

const defaultPublishTargets = (): PublishTargetConfig[] => [defaultPublishTarget(1)];

const emptyBundle = (): ConfigBundle => ({
  config: {
    platforms: [],
    publish_platform: "wechat",
    img_api: defaultImageApiConfig(),
    publish_targets: defaultPublishTargets(),
    use_template: true,
    template_category: "general",
    template_name: "magazine",
    use_compress: false,
    aiforge_search_max_results: 8,
    aiforge_search_min_results: 3,
    min_article_len: 900,
    max_article_len: 2000,
    auto_publish: false,
    article_format: "html",
    format_publish: true
  },
  aiforge_config: {
    default_search_provider: "google_news_rss",
    search: {
      provider: "google_news_rss",
      max_results: 8,
      request_timeout_secs: 15,
      locale: "zh-CN"
    },
    fetcher: {
      user_agent: "postpub/0.1",
      request_timeout_secs: 15,
      max_content_chars: 8000
    }
  },
  ui_config: {
    theme: DEFAULT_THEME,
    window_mode: DEFAULT_WINDOW_MODE,
    design_theme: DEFAULT_DESIGN_THEME,
    page_design: defaultPageDesign(),
    custom_llm_providers: [
      {
        id: "custom-1",
        name: "Custom",
        api_key: "",
        api_base: "https://api.openai.com/v1",
        model: "gpt-4o-mini",
        protocol_type: "openai",
        max_tokens: 8192,
        enabled: true
      }
    ]
  }
});

function normalizeImageApiConfig(imageApi?: Partial<ImageApiConfig> | null): ImageApiConfig {
  const defaults = defaultImageApiConfig();
  const legacyApiType = imageApi?.api_type === "ali" ? "ali" : "picsum";
  const providers = imageApi?.providers?.length
    ? imageApi.providers.map((provider, index) =>
        defaultImageModelProvider(provider.provider_type || "picsum", index + 1, {
          ...provider,
          id: provider.id || `image-${provider.provider_type || "picsum"}-${index + 1}`,
          name:
            provider.name ||
            defaultImageModelProvider(provider.provider_type || "picsum", index + 1).name
        })
      )
    : [
        defaultImageModelProvider("picsum", 1, {
          id: "image-picsum-1",
          name: "Picsum",
          api_key: imageApi?.picsum?.api_key ?? defaults.picsum.api_key,
          model: imageApi?.picsum?.model ?? defaults.picsum.model,
          enabled: legacyApiType === "picsum"
        }),
        defaultImageModelProvider("ali", 1, {
          id: "image-ali-1",
          name: "阿里万相",
          api_key: imageApi?.ali?.api_key ?? defaults.ali.api_key,
          model: imageApi?.ali?.model ?? defaults.ali.model,
          enabled: legacyApiType === "ali"
        })
      ];
  const activeProvider =
    providers.find((provider) => provider.id === imageApi?.active_provider_id) ||
    providers.find((provider) => provider.enabled) ||
    providers[0] ||
    defaultImageApiConfig().providers[0];

  return {
    active_provider_id: activeProvider.id,
    providers: providers.map((provider) => ({
      ...provider,
      enabled: provider.id === activeProvider.id ? true : provider.enabled
    })),
    api_type: activeProvider.provider_type,
    ali: defaultImageApiProvider({
      ...defaults.ali,
      api_key: providers.find((provider) => provider.provider_type === "ali")?.api_key ?? imageApi?.ali?.api_key,
      model: providers.find((provider) => provider.provider_type === "ali")?.model ?? imageApi?.ali?.model
    }),
    picsum: defaultImageApiProvider({
      ...defaults.picsum,
      api_key:
        providers.find((provider) => provider.provider_type === "picsum")?.api_key ??
        imageApi?.picsum?.api_key,
      model:
        providers.find((provider) => provider.provider_type === "picsum")?.model ??
        imageApi?.picsum?.model
    })
  };
}

function normalizePublishTarget(target?: Partial<PublishTargetConfig> | null, index = 1): PublishTargetConfig {
  return defaultPublishTarget(index, {
    ...(target || {}),
    id: target?.id || `publish-wechat-${index}`,
    name: target?.name || defaultPublishTarget(index).name,
    platform_type: target?.platform_type || "wechat"
  });
}

function normalizeCustomLlmProviders(providers?: CustomLlmProvider[] | null): CustomLlmProvider[] {
  const nextProviders = providers?.length
    ? providers
    : emptyBundle().ui_config.custom_llm_providers;
  const activeProvider = nextProviders.find((provider) => provider.enabled) || nextProviders[0];

  return nextProviders.map((provider) => ({
    ...provider,
    protocol_type:
      provider.protocol_type?.trim() === "custom"
        ? "openai_compatible"
        : provider.protocol_type?.trim() || "openai",
    enabled: provider.id === activeProvider.id
  }));
}

function normalizePostpubConfig(config: PostpubConfig): PostpubConfig {
  const publishTargets = config.publish_targets?.length
    ? config.publish_targets.map((target, index) => normalizePublishTarget(target, index + 1))
    : [];

  return {
    ...config,
    img_api: normalizeImageApiConfig(config.img_api),
    publish_targets: publishTargets
  };
}

function normalizePageDesign(pageDesign?: Partial<PageDesignConfig> | null): PageDesignConfig {
  return {
    ...defaultPageDesign(),
    ...(pageDesign || {})
  };
}

function normalizeUiConfig(uiConfig: UiConfig): UiConfig {
  return {
    ...uiConfig,
    theme: uiConfig.theme === "dark" ? "dark" : DEFAULT_THEME,
    window_mode: uiConfig.window_mode?.toUpperCase() === "MAXIMIZED" ? "MAXIMIZED" : DEFAULT_WINDOW_MODE,
    design_theme: uiConfig.design_theme === "default" ? "default" : DEFAULT_DESIGN_THEME,
    page_design: normalizePageDesign(uiConfig.page_design),
    custom_llm_providers: normalizeCustomLlmProviders(uiConfig.custom_llm_providers)
  };
}

function normalizeBundle(bundle: ConfigBundle): ConfigBundle {
  return {
    ...bundle,
    config: normalizePostpubConfig(bundle.config),
    ui_config: normalizeUiConfig(bundle.ui_config)
  };
}

function resolveDesignSurfaceTheme(uiConfig: UiConfig): "light" | "dark" {
  if (uiConfig.design_theme !== "follow-system") {
    return "light";
  }

  return systemThemeMediaQuery?.matches ? "dark" : "light";
}

function applyUiConfig(uiConfig: UiConfig) {
  if (typeof document === "undefined") {
    return;
  }

  const normalized = normalizeUiConfig(uiConfig);
  const windowMode = normalized.window_mode.toLowerCase();

  document.documentElement.dataset.theme = normalized.theme;
  document.documentElement.dataset.windowMode = windowMode;
  document.documentElement.dataset.designTheme = normalized.design_theme;
  document.documentElement.dataset.designSurface = resolveDesignSurfaceTheme(normalized);
  document.documentElement.dataset.useOriginalStyles = String(normalized.page_design.use_original_styles);

  document.documentElement.style.setProperty(
    "--designer-container-max-width",
    `${normalized.page_design.container_max_width}px`
  );
  document.documentElement.style.setProperty(
    "--designer-container-margin-h",
    `${normalized.page_design.container_margin_h}px`
  );
  document.documentElement.style.setProperty(
    "--designer-container-bg",
    normalized.page_design.container_bg_color
  );
  document.documentElement.style.setProperty(
    "--designer-card-radius",
    `${normalized.page_design.card_border_radius}px`
  );
  document.documentElement.style.setProperty(
    "--designer-card-padding",
    `${normalized.page_design.card_padding}px`
  );
  document.documentElement.style.setProperty("--designer-card-bg", normalized.page_design.card_bg_color);
  document.documentElement.style.setProperty(
    "--designer-card-shadow",
    normalized.page_design.card_box_shadow
  );
  document.documentElement.style.setProperty(
    "--designer-font-size",
    `${normalized.page_design.typography_font_size}px`
  );
  document.documentElement.style.setProperty(
    "--designer-line-height",
    String(normalized.page_design.typography_line_height)
  );
  document.documentElement.style.setProperty(
    "--designer-heading-scale",
    String(normalized.page_design.typography_heading_scale)
  );
  document.documentElement.style.setProperty(
    "--designer-text-color",
    normalized.page_design.typography_text_color
  );
  document.documentElement.style.setProperty(
    "--designer-heading-color",
    normalized.page_design.typography_heading_color
  );
  document.documentElement.style.setProperty(
    "--designer-section-gap",
    `${normalized.page_design.spacing_section_margin}px`
  );
  document.documentElement.style.setProperty(
    "--designer-element-gap",
    `${normalized.page_design.spacing_element_margin}px`
  );
  document.documentElement.style.setProperty(
    "--designer-accent-primary",
    normalized.page_design.accent_primary_color
  );
  document.documentElement.style.setProperty(
    "--designer-accent-secondary",
    normalized.page_design.accent_secondary_color
  );
  document.documentElement.style.setProperty(
    "--designer-accent-highlight",
    normalized.page_design.accent_highlight_bg
  );

  document.body.classList.remove("window-mode-standard", "window-mode-maximized");
  document.body.classList.add(`window-mode-${windowMode}`);
  document.body.dataset.windowMode = windowMode;
}

export const useConfigStore = defineStore("config", {
  state: () => ({
    bundle: emptyBundle(),
    designSurface: "light" as "light" | "dark",
    loading: false,
    saving: false,
    error: "",
    lastMessage: ""
  }),

  actions: {
    applyUiConfig(uiConfig?: UiConfig) {
      const normalized = normalizeUiConfig(uiConfig ?? this.bundle.ui_config);
      this.designSurface = resolveDesignSurfaceTheme(normalized);
      applyUiConfig(normalized);
    },

    bindSystemThemeListener() {
      if (typeof window === "undefined" || systemThemeListenerBound || !window.matchMedia) {
        return;
      }

      systemThemeMediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
      systemThemeMediaQuery.addEventListener("change", () => {
        this.applyUiConfig(this.bundle.ui_config);
      });
      systemThemeListenerBound = true;
    },

    initialize() {
      this.bindSystemThemeListener();
      this.applyUiConfig(this.bundle.ui_config);

      if (!this.loading) {
        void this.load();
      }
    },

    async load() {
      this.loading = true;
      this.error = "";
      this.lastMessage = "";

      try {
        const response = await apiGet<ApiResponse<ConfigBundle>>("/api/config");
        this.bundle = normalizeBundle(response.data);
        this.applyUiConfig(this.bundle.ui_config);
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
    },

    async loadDefaults() {
      this.loading = true;
      this.error = "";
      this.lastMessage = "";

      try {
        const response = await apiGet<ApiResponse<ConfigBundle>>("/api/config/default");
        this.bundle = normalizeBundle(response.data);
        this.applyUiConfig(this.bundle.ui_config);
        this.lastMessage = translate("messages.config.loadedDefaults");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
    },

    async save() {
      this.saving = true;
      this.error = "";
      this.lastMessage = "";

      try {
        this.bundle = normalizeBundle(this.bundle);
        this.applyUiConfig(this.bundle.ui_config);
        const response = await apiPut<ApiResponse<ConfigBundle>>("/api/config", this.bundle);
        this.bundle = normalizeBundle(response.data);
        this.applyUiConfig(this.bundle.ui_config);
        this.lastMessage = translate("messages.config.saved");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.saving = false;
      }
    }
  }
});
