import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useConfigStore } from "./config";
import { i18n } from "../utils/i18n";

function jsonResponse(data: unknown): Response {
  return new Response(JSON.stringify(data), {
    status: 200,
    headers: { "Content-Type": "application/json" }
  });
}

describe("config store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
    window.localStorage.setItem("locale", "en-US");
    i18n.global.locale.value = "en-US";
  });

  it("loads and saves the config bundle", async () => {
    const fetchMock = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValueOnce(
        jsonResponse({
          success: true,
          data: {
            config: {
              platforms: [],
              publish_platform: "web",
              img_api: {
                api_type: "picsum",
                ali: {
                  api_key: "",
                  model: "wanx2.0-t2i-turbo"
                },
                picsum: {
                  api_key: "",
                  model: ""
                }
              },
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
              theme: "light",
              window_mode: "STANDARD",
              design_theme: "follow-system",
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
          }
        })
      )
      .mockResolvedValueOnce(
        jsonResponse({
          success: true,
          message: "configuration saved",
          data: {
            config: {
              platforms: [],
              publish_platform: "desktop",
              img_api: {
                api_type: "ali",
                ali: {
                  api_key: "ali-key",
                  model: "wanx2.1-t2i-plus"
                },
                picsum: {
                  api_key: "",
                  model: ""
                }
              },
              use_template: true,
              template_category: "general",
              template_name: "magazine",
              use_compress: false,
              aiforge_search_max_results: 8,
              aiforge_search_min_results: 1,
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
              theme: "light",
              window_mode: "STANDARD",
              design_theme: "follow-system",
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
          }
        })
      );

    const store = useConfigStore();
    await store.load();

    expect(store.bundle.config.publish_platform).toBe("web");
    expect(store.bundle.config.img_api.api_type).toBe("picsum");
    expect(store.bundle.ui_config.window_mode).toBe("STANDARD");
    expect(document.documentElement.dataset.windowMode).toBe("standard");

    store.bundle.config.publish_platform = "desktop";
    store.bundle.config.aiforge_search_min_results = 1;
    store.bundle.config.img_api.api_type = "ali";
    store.bundle.config.img_api.ali.api_key = "ali-key";
    store.bundle.config.img_api.ali.model = "wanx2.1-t2i-plus";
    await store.save();

    expect(store.bundle.config.publish_platform).toBe("desktop");
    expect(store.bundle.config.img_api.api_type).toBe("ali");
    expect(store.lastMessage).toBe("Saved configuration");
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("normalizes custom llm provider protocol and max tokens", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValueOnce(
      jsonResponse({
        success: true,
        data: {
          config: {
            platforms: [],
            publish_platform: "web",
            img_api: {
              api_type: "picsum",
              ali: {
                api_key: "",
                model: "wanx2.0-t2i-turbo"
              },
              picsum: {
                api_key: "",
                model: ""
              }
            },
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
            theme: "light",
            window_mode: "STANDARD",
            design_theme: "follow-system",
            custom_llm_providers: [
              {
                id: "custom-1",
                name: "Custom",
                api_key: "",
                api_base: "https://api.openai.com/v1",
                model: "gpt-4o-mini",
                protocol_type: "custom",
                max_tokens: 999999,
                enabled: true
              }
            ]
          }
        }
      })
    );

    const store = useConfigStore();
    await store.load();

    expect(store.bundle.ui_config.custom_llm_providers[0].protocol_type).toBe(
      "openai_compatible"
    );
    expect(store.bundle.ui_config.custom_llm_providers[0].max_tokens).toBe(
      131072
    );
  });
});
