import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import router from "./router";
import App from "./App.vue";
import { i18n } from "./utils/i18n";

function jsonResponse(data: unknown): Response {
  return new Response(JSON.stringify(data), {
    status: 200,
    headers: { "Content-Type": "application/json" }
  });
}

describe("App shell", () => {
  beforeEach(async () => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
    window.localStorage.setItem("locale", "en-US");
    i18n.global.locale.value = "en-US";
    vi.spyOn(globalThis, "fetch")
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
                  key_name: "CUSTOM_API_KEY",
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
          data: {
            service: "postpub-api",
            status: "ok",
            version: "0.1.0",
            timestamp: "2026-03-28T00:00:00Z"
          }
        })
      )
      .mockResolvedValueOnce(
        jsonResponse({
          success: true,
          data: {
            app_root: "D:/work/github/postpub",
            config_dir: "D:/work/github/postpub/config",
            articles_dir: "D:/work/github/postpub/output/article",
            templates_dir: "D:/work/github/postpub/templates",
            images_dir: "D:/work/github/postpub/images",
            logs_dir: "D:/work/github/postpub/logs",
            temp_dir: "D:/work/github/postpub/.tmp",
            config_file: "D:/work/github/postpub/config.yaml",
            aiforge_config_file: "D:/work/github/postpub/aiforge.toml",
            ui_config_file: "D:/work/github/postpub/ui_config.json",
            publish_records_file: "D:/work/github/postpub/publish_records.json"
          }
        })
      );

    await router.push("/");
    await router.isReady();
  });

  it("renders the shell without the removed docs shortcut", async () => {
    const wrapper = mount(App, {
      global: {
        plugins: [createPinia(), router, i18n]
      }
    });

    await flushPromises();

    expect(wrapper.text()).toContain("postpub");
    expect(wrapper.text()).toContain("postpub-api");
    expect(document.documentElement.dataset.windowMode).toBe("standard");
    expect(wrapper.text()).not.toContain("Open docs route");
    expect(wrapper.text()).not.toContain("打开文档路由");
  });
});
