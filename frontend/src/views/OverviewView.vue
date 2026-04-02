<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { hostBridge } from "../host";
import { useAppStore } from "../stores/app";
import { useArticleStore } from "../stores/articles";
import { useGenerationStore } from "../stores/generation";
import { useTemplateStore } from "../stores/templates";

const appStore = useAppStore();
const articleStore = useArticleStore();
const generationStore = useGenerationStore();
const templateStore = useTemplateStore();
const environment = computed(() => hostBridge.getEnvironmentLabel());
const { t } = useI18n();

const metricCards = computed(() => [
  {
    label: t("overview.metrics.templates"),
    value: templateStore.templates.length,
    note: t("overview.metrics.templateNote", { count: templateStore.categories.length })
  },
  {
    label: t("overview.metrics.articles"),
    value: articleStore.articles.length,
    note: t("overview.metrics.articleNote")
  },
  {
    label: t("overview.metrics.tasks"),
    value: generationStore.tasks.length,
    note: generationStore.current?.status || t("overview.metrics.taskNote")
  },
  {
    label: t("overview.metrics.runtime"),
    value: environment.value,
    note: appStore.health?.service || "postpub-api"
  }
]);

onMounted(() => {
  if (!appStore.health && !appStore.loading) {
    void appStore.refreshSystem();
  }
  if (!templateStore.categories.length && !templateStore.loading) {
    void templateStore.loadAll();
  }
  if (!articleStore.articles.length && !articleStore.loading) {
    void articleStore.loadAll();
  }
  if (!generationStore.tasks.length && !generationStore.loading) {
    void generationStore.loadTasks();
  }
});
</script>

<template>
  <section class="hero-panel">
    <div>
      <p class="eyebrow">{{ t("overview.workspaceEyebrow") }}</p>
      <h3 class="hero-title">{{ t("overview.heroTitle") }}</h3>
      <p class="hero-copy">
        {{ t("overview.heroCopy") }}
      </p>
    </div>
    <div class="btn-row">
      <button class="btn btn-primary" type="button" @click="appStore.refreshSystem">
        {{ appStore.loading ? t("common.refreshing") : t("overview.refreshRuntime") }}
      </button>
    </div>
  </section>

  <section class="metric-grid">
    <article v-for="card in metricCards" :key="card.label" class="metric-card">
      <p class="metric-label">{{ card.label }}</p>
      <strong class="metric-value">{{ card.value }}</strong>
      <p class="metric-note">{{ card.note }}</p>
    </article>
  </section>

  <section class="content-grid">
    <article class="panel">
      <h3 class="panel-title">{{ t("overview.implementedTitle") }}</h3>
      <ul class="clean-list">
        <li>{{ t("overview.implementedItems.one") }}</li>
        <li>{{ t("overview.implementedItems.two") }}</li>
        <li>{{ t("overview.implementedItems.three") }}</li>
        <li>{{ t("overview.implementedItems.four") }}</li>
        <li>{{ t("overview.implementedItems.five") }}</li>
      </ul>
    </article>

    <article class="panel">
      <h3 class="panel-title">{{ t("overview.constraintsTitle") }}</h3>
      <ul class="clean-list">
        <li>{{ t("overview.constraintItems.one") }}</li>
        <li>{{ t("overview.constraintItems.two") }}</li>
        <li>{{ t("overview.constraintItems.three") }}</li>
      </ul>
    </article>
  </section>

  <section v-if="appStore.paths" class="panel">
    <h3 class="panel-title">{{ t("overview.pathsTitle") }}</h3>
    <div class="info-grid">
      <div class="info-row">
        <span>{{ t("overview.paths.appRoot") }}</span>
        <code>{{ appStore.paths.app_root }}</code>
      </div>
      <div class="info-row">
        <span>{{ t("overview.paths.configDir") }}</span>
        <code>{{ appStore.paths.config_dir }}</code>
      </div>
      <div class="info-row">
        <span>{{ t("overview.paths.templatesDir") }}</span>
        <code>{{ appStore.paths.templates_dir }}</code>
      </div>
      <div class="info-row">
        <span>{{ t("overview.paths.articlesDir") }}</span>
        <code>{{ appStore.paths.articles_dir }}</code>
      </div>
      <div class="info-row">
        <span>{{ t("overview.paths.imagesDir") }}</span>
        <code>{{ appStore.paths.images_dir }}</code>
      </div>
      <div class="info-row">
        <span>{{ t("overview.paths.publishRecordsFile") }}</span>
        <code>{{ appStore.paths.publish_records_file }}</code>
      </div>
    </div>
  </section>
</template>
