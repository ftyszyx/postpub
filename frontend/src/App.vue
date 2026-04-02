<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { RouterLink, RouterView, useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import { useAppStore } from "./stores/app";
import { useConfigStore } from "./stores/config";
import { useLocaleStore } from "./stores/locale";
import { useNavigationStore, type ConfigPanel } from "./stores/navigation";
import { supportedLocales, type SupportedLocale } from "./utils/i18n";

interface ConfigSubnavItem {
  panel: ConfigPanel;
  label: string;
}

interface NavItem {
  to: string;
  label: string;
  children?: ConfigSubnavItem[];
}

const route = useRoute();
const router = useRouter();
const appStore = useAppStore();
const configStore = useConfigStore();
const localeStore = useLocaleStore();
const navigationStore = useNavigationStore();
const { t } = useI18n();

const configMenuExpanded = ref(false);
const isConfigRoute = computed(() => route.name === "config");
const currentConfigPanel = computed<ConfigPanel>(() => navigationStore.activeConfigPanel);

const configSubnavItems = computed<ConfigSubnavItem[]>(() => [
  { panel: "ui", label: t("config.panels.ui") },
  { panel: "image-models", label: t("config.panels.imageModels") },
  { panel: "llm-models", label: t("config.panels.llmModels") },
  { panel: "publish-platforms", label: t("config.panels.publishPlatforms") }
]);

const navItems = computed<NavItem[]>(() => [
  { to: "/", label: t("nav.overview") },
  { to: "/generation", label: t("nav.generation") },
  { to: "/templates", label: t("nav.templates") },
  { to: "/articles", label: t("nav.articles") },
  {
    to: "/config",
    label: t("nav.config"),
    children: configSubnavItems.value
  }
]);

const localeLabels = computed<Record<SupportedLocale, string>>(() => ({
  "zh-CN": t("common.localeZhCn"),
  "en-US": t("common.localeEnUs")
}));

watch(
  () => configStore.bundle.ui_config,
  (uiConfig) => {
    configStore.applyUiConfig(uiConfig);
  },
  { deep: true }
);

watch(
  isConfigRoute,
  (value) => {
    if (value) {
      configMenuExpanded.value = true;
    }
  },
  { immediate: true }
);

function onLocaleChange(event: Event) {
  localeStore.setLocale((event.target as HTMLSelectElement).value as SupportedLocale);
}

async function toggleConfigMenu() {
  if (!isConfigRoute.value) {
    configMenuExpanded.value = true;
    navigationStore.setActiveConfigPanel("ui");
    await router.push("/config");
    return;
  }

  configMenuExpanded.value = !configMenuExpanded.value;
}

async function openConfigPanel(panel: ConfigPanel) {
  navigationStore.setActiveConfigPanel(panel);
  configMenuExpanded.value = true;

  if (!isConfigRoute.value) {
    await router.push("/config");
  }
}

onMounted(() => {
  configStore.initialize();

  if (!appStore.health && !appStore.loading) {
    void appStore.refreshSystem();
  }
});
</script>

<template>
  <main class="desktop-shell desktop-shell--no-preview">
    <header class="app-header">
      <RouterLink to="/" class="brand-link">
        <div class="brand-mark" aria-hidden="true">
          <svg class="brand-mark__svg" viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg">
            <defs>
              <linearGradient id="postpub-brand-gradient" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" stop-color="#00e5ff" />
                <stop offset="100%" stop-color="#7c3aed" />
              </linearGradient>
            </defs>
            <circle class="orbit orbit-a" cx="32" cy="32" r="27" />
            <circle class="orbit orbit-b" cx="32" cy="32" r="18" />
            <path
              class="brand-x"
              d="M24 24L40 40M40 24L24 40"
              stroke="url(#postpub-brand-gradient)"
              stroke-width="6"
              stroke-linecap="round"
            />
            <circle cx="32" cy="32" r="3.5" fill="#ffffff" />
          </svg>
        </div>
        <div class="brand-copy">
          <span class="brand-title">postpub</span>
          <span class="brand-subtitle">{{ t("app.brandCopy") }}</span>
        </div>
      </RouterLink>
    </header>

    <aside class="app-sidebar">
      <nav class="sidebar-group" aria-label="Primary">
        <template v-for="item in navItems" :key="item.to">
          <div v-if="item.children" class="sidebar-item-group" :class="{ 'sidebar-item-group--open': configMenuExpanded }">
            <button
              class="sidebar-link sidebar-link--toggle"
              :class="{ 'sidebar-link--current': isConfigRoute }"
              type="button"
              @click="toggleConfigMenu"
            >
              <span class="sidebar-link__copy">
                <span class="sidebar-link__label">{{ item.label }}</span>
              </span>
              <svg class="sidebar-link__arrow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="9,18 15,12 9,6" />
              </svg>
            </button>

            <div v-if="configMenuExpanded" class="sidebar-subnav">
              <button
                v-for="child in item.children"
                :key="child.panel"
                class="sidebar-sublink"
                type="button"
                :class="{ 'sidebar-sublink--active': currentConfigPanel === child.panel }"
                @click="openConfigPanel(child.panel)"
              >
                {{ child.label }}
              </button>
            </div>
          </div>

          <RouterLink v-else :to="item.to" class="sidebar-link">
            <span class="sidebar-link__copy">
              <span class="sidebar-link__label">{{ item.label }}</span>
            </span>
          </RouterLink>
        </template>
      </nav>

      <section class="sidebar-meta">
        <div class="sidebar-meta__rows">
          <div class="meta-row">
            <span>{{ t("common.api") }}</span>
            <strong>{{ appStore.health?.service || "postpub-api" }}</strong>
          </div>
          <div class="meta-row">
            <span>{{ t("common.status") }}</span>
            <strong>{{ appStore.health?.status || t("common.checking") }}</strong>
          </div>
          <div class="meta-row">
            <span>{{ t("common.version") }}</span>
            <strong>{{ appStore.health?.version || "-" }}</strong>
          </div>
        </div>

        <label class="field field--compact">
          <span>{{ t("common.language") }}</span>
          <select class="text-input" :value="localeStore.current" @change="onLocaleChange">
            <option v-for="locale in supportedLocales" :key="locale" :value="locale">
              {{ localeLabels[locale] }}
            </option>
          </select>
        </label>
      </section>
    </aside>

    <section class="workspace-shell">
      <header class="workspace-header">
        <div class="workspace-title-block">
          <p class="eyebrow">{{ t("app.brandEyebrow") }}</p>
          <h2 class="workspace-title">postpub</h2>
          <p class="workspace-copy">{{ t("app.brandCopy") }}</p>
        </div>

        <div class="workspace-actions">
          <button class="btn btn-secondary" type="button" @click="appStore.refreshSystem">
            {{ appStore.loading ? t("common.refreshing") : t("common.refreshSystem") }}
          </button>
        </div>
      </header>

      <div v-if="appStore.error" class="banner banner-error">
        {{ appStore.error }}
      </div>

      <div class="workspace-scroll">
        <RouterView />
      </div>
    </section>
  </main>
</template>
