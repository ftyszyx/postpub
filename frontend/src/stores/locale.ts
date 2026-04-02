import { defineStore } from "pinia";
import { ref, watch } from "vue";
import { i18n, readInitialLocale, type SupportedLocale } from "../utils/i18n";

export const useLocaleStore = defineStore("locale", () => {
  const current = ref<SupportedLocale>(readInitialLocale());

  const setLocale = (locale: SupportedLocale) => {
    current.value = locale;
  };

  watch(
    current,
    (locale) => {
      window.localStorage.setItem("locale", locale);
      i18n.global.locale.value = locale;
      document.documentElement.lang = locale;
    },
    { immediate: true }
  );

  return {
    current,
    setLocale
  };
});
