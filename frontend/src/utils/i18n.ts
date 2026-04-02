import { createI18n } from "vue-i18n";
import enUS from "../locales/en-US.json";
import zhCN from "../locales/zh-CN.json";

export const supportedLocales = ["zh-CN", "en-US"] as const;
export type SupportedLocale = (typeof supportedLocales)[number];

export function readInitialLocale(): SupportedLocale {
  const saved = window.localStorage.getItem("locale");
  if (saved === "en-US" || saved === "zh-CN") {
    return saved;
  }

  const browserLocale = window.navigator.language.toLowerCase();
  if (browserLocale.startsWith("zh")) {
    return "zh-CN";
  }

  return "en-US";
}

export const i18n = createI18n({
  legacy: false,
  locale: readInitialLocale(),
  fallbackLocale: "en-US",
  messages: {
    "zh-CN": zhCN,
    "en-US": enUS
  }
});

export function translate(key: string, values?: Record<string, unknown>): string {
  return String(i18n.global.t(key, values || {}));
}
