<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, reactive, ref, watch } from "vue";
import { marked } from "marked";
import { useI18n } from "vue-i18n";
import { apiDelete, apiGet, encodePathSegments, type ApiResponse } from "../api/client";
import TemplateMonacoEditor from "../components/TemplateMonacoEditor.vue";
import { useArticleStore } from "../stores/articles";
import { useConfigStore } from "../stores/config";
import type { ArticleDocument, ArticleSummary, ArticleVariantDocument } from "../types/postpub";
import { decoratePreviewDocument } from "../utils/preview";

type ArticleLayout = "grid" | "list";
type ArticleStatusFilter = "all" | "published" | "failed" | "unpublished";

const articleStore = useArticleStore();
const configStore = useConfigStore();
const { t, locale } = useI18n();

const layout = ref<ArticleLayout>("grid");
const currentStatus = ref<ArticleStatusFilter>("all");
const searchKeyword = ref("");
const batchMode = ref(false);
const selectedPaths = ref<string[]>([]);
const articleEditorOpen = ref(false);
const articleEditorDraft = ref("");
const articleEditorSelectedSourceKey = ref("original");
const articleMonacoEditor = ref<{ focus: () => void } | null>(null);
const articleEditorWorkspace = ref<HTMLElement | null>(null);
const articleEditorPreviewRefreshKey = ref(0);
const articleEditorFullscreen = ref(false);
const articleEditorCursor = reactive({ line: 1, column: 1 });
const articleEditorCodePanelWidth = ref(0.68);

const previewHtmlMap = reactive<Record<string, string>>({});
const previewStateMap = reactive<Record<string, "idle" | "loading" | "ready" | "error">>({});

let articleEditorResizeCleanup: (() => void) | null = null;

const sortedArticles = computed(() =>
  [...articleStore.articles].sort((left, right) => right.updated_at.localeCompare(left.updated_at))
);

const filteredArticles = computed(() => {
  const keyword = searchKeyword.value.trim().toLowerCase();
  return sortedArticles.value.filter((article) => {
    const status = normalizeStatus(article.status);
    if (currentStatus.value !== "all" && status !== currentStatus.value) {
      return false;
    }

    if (!keyword) {
      return true;
    }

    return [article.title, article.name, article.relative_path, article.format, String(article.variant_count || 0)]
      .filter(Boolean)
      .some((value) => value.toLowerCase().includes(keyword));
  });
});

const statusOptions = computed(() => [
  {
    key: "all" as const,
    label: t("articles.filters.all"),
    count: articleStore.articles.length
  },
  {
    key: "published" as const,
    label: t("articles.filters.published"),
    count: articleStore.articles.filter((article) => normalizeStatus(article.status) === "published").length
  },
  {
    key: "failed" as const,
    label: t("articles.filters.failed"),
    count: articleStore.articles.filter((article) => normalizeStatus(article.status) === "failed").length
  },
  {
    key: "unpublished" as const,
    label: t("articles.filters.unpublished"),
    count: articleStore.articles.filter((article) => normalizeStatus(article.status) === "unpublished").length
  }
]);

const articleEditorSourceOptions = computed(() => {
  const current = articleStore.current;
  if (!current) return [];

  return [
    {
      key: "original",
      label: t("articles.originalVersion"),
      kind: "original" as const,
      format: current.summary.format
    },
    ...current.variants.map((variant) => ({
      key: buildVariantSourceKey(variant),
      label: t("articles.variantOptionLabel", {
        target: variant.summary.target_name,
        format: variant.summary.format
      }),
      kind: "variant" as const,
      format: variant.summary.format
    }))
  ];
});

const articleEditorActiveVariant = computed(() => {
  const current = articleStore.current;
  if (!current || articleEditorSelectedSourceKey.value === "original") return null;

  return (
    current.variants.find((variant) => buildVariantSourceKey(variant) === articleEditorSelectedSourceKey.value) ?? null
  );
});

const articleEditorActiveSource = computed(() => {
  const selected = articleEditorSourceOptions.value.find((option) => option.key === articleEditorSelectedSourceKey.value);
  return selected ?? articleEditorSourceOptions.value[0] ?? null;
});

const articleEditorActiveSourceLabel = computed(() => articleEditorActiveSource.value?.label || t("articles.originalVersion"));

const articleEditorIsReadonly = computed(() => Boolean(articleEditorActiveVariant.value));

const articleEditorContent = computed({
  get() {
    return articleEditorActiveVariant.value?.content ?? articleEditorDraft.value;
  },
  set(value: string) {
    if (!articleEditorIsReadonly.value) {
      articleEditorDraft.value = value;
    }
  }
});

const articleEditorLanguage = computed<"html" | "markdown" | "plaintext">(() =>
  normalizeEditorLanguage(articleEditorActiveSource.value?.format || articleStore.current?.summary.format || "html")
);

const articleEditorPreviewDocument = computed(() => {
  if (!articleStore.current) return "";
  if (articleEditorActiveVariant.value) {
    return buildVariantPreviewDocument(articleEditorActiveVariant.value);
  }

  const title = articleStore.current.summary.title || articleStore.current.summary.name || t("articles.previewTitle");
  const source = buildArticlePreviewSource(articleEditorDraft.value, articleStore.current.summary.format);
  return decoratePreviewDocument(source, configStore.designSurface, title);
});

function buildVariantPreviewDocument(variant: ArticleVariantDocument) {
  const articleTitle =
    articleStore.current?.summary.title || articleStore.current?.summary.name || t("articles.previewTitle");
  const source = variant.preview_html || buildArticlePreviewSource(variant.content, variant.summary.format);
  return decoratePreviewDocument(
    source,
    configStore.designSurface,
    `${articleTitle} - ${variant.summary.target_name}`
  );
}

function buildVariantSourceKey(variant: ArticleVariantDocument) {
  return `variant:${variant.summary.target_id}:${variant.summary.format}`;
}

function normalizeEditorLanguage(format: string) {
  const normalized = format.toLowerCase();
  if (normalized.includes("markdown") || normalized === "md") {
    return "markdown";
  }

  if (normalized.includes("text") || normalized === "txt") {
    return "plaintext";
  }

  return "html";
}

function normalizeStatus(status: string) {
  const normalized = status.toLowerCase();
  if (normalized === "published" || normalized === "failed") {
    return normalized;
  }

  return "unpublished";
}

function selectedPathSet() {
  return new Set(selectedPaths.value);
}

function isSelected(relativePath: string) {
  return selectedPathSet().has(relativePath);
}

function toggleSelected(relativePath: string) {
  const next = new Set(selectedPaths.value);
  if (next.has(relativePath)) {
    next.delete(relativePath);
  } else {
    next.add(relativePath);
  }
  selectedPaths.value = [...next];
}

function toggleBatchMode() {
  batchMode.value = !batchMode.value;
  if (!batchMode.value) {
    selectedPaths.value = [];
  }
}

function formatRelativeTime(value: string) {
  const date = new Date(value);
  const today = new Date();
  const diffDays = Math.floor((today.getTime() - date.getTime()) / (1000 * 60 * 60 * 24));
  if (Number.isNaN(date.getTime())) return value;
  if (diffDays <= 0) return t("templates.relativeToday");
  if (diffDays === 1) return t("templates.relativeYesterday");
  if (diffDays < 7) return t("templates.relativeDaysAgo", { days: diffDays });
  return date.toLocaleDateString(locale.value, { month: "2-digit", day: "2-digit" });
}

function formatBytes(sizeBytes: number) {
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  const units = ["KB", "MB", "GB"];
  let size = sizeBytes;
  let unitIndex = -1;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  return `${size.toFixed(size >= 10 ? 0 : 2)} ${units[unitIndex]}`;
}

function statusClass(status: string) {
  const normalized = normalizeStatus(status);
  return {
    published: normalized === "published",
    failed: normalized === "failed",
    unpublished: normalized === "unpublished"
  };
}

function statusText(status: string) {
  const normalized = normalizeStatus(status);
  return t(`articles.filters.${normalized}`);
}

function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function renderPlainTextPreview(content: string) {
  return content
    .split("\n")
    .map((line) => (line.trim() ? `<p>${escapeHtml(line)}</p>` : "<br />"))
    .join("");
}

function buildArticlePreviewSource(content: string, format: string) {
  const normalized = format.toLowerCase();

  if (normalized.includes("markdown") || normalized === "md") {
    marked.setOptions({ gfm: true, breaks: true, async: false });
    return marked.parse(content) as string;
  }

  if (normalized.includes("text") || normalized === "txt") {
    return renderPlainTextPreview(content);
  }

  return content;
}

async function ensurePreview(relativePath: string) {
  if (previewHtmlMap[relativePath] || previewStateMap[relativePath] === "loading") return;

  previewStateMap[relativePath] = "loading";

  try {
    const response = await apiGet<ApiResponse<ArticleDocument>>(`/api/articles/${encodePathSegments(relativePath)}`);
    const article = response.data;
    previewHtmlMap[relativePath] = decoratePreviewDocument(
      article.preview_html || buildArticlePreviewSource(article.content, article.summary.format),
      configStore.designSurface,
      article.summary.title || article.summary.name
    );
    previewStateMap[relativePath] = "ready";
  } catch {
    previewStateMap[relativePath] = "error";
  }
}

function resetPreview(relativePath: string) {
  delete previewHtmlMap[relativePath];
  delete previewStateMap[relativePath];
}

async function openArticleEditor(relativePath: string) {
  await articleStore.open(relativePath);
  if (!articleStore.current) return;
  articleEditorDraft.value = articleStore.current.content;
  articleEditorSelectedSourceKey.value = "original";
  articleEditorPreviewRefreshKey.value += 1;
  resetArticleEditorViewport();
  articleEditorOpen.value = true;
}

function closeArticleEditor() {
  articleEditorOpen.value = false;
  articleEditorFullscreen.value = false;
}

async function saveArticleEditor() {
  if (!articleStore.current || articleEditorIsReadonly.value) return;
  articleStore.current.content = articleEditorDraft.value;
  await articleStore.saveCurrent();
  if (!articleStore.error) {
    resetPreview(articleStore.current.summary.relative_path);
    void ensurePreview(articleStore.current.summary.relative_path);
    closeArticleEditor();
  }
}

function resetArticleEditorViewport() {
  articleEditorCursor.line = 1;
  articleEditorCursor.column = 1;
  articleEditorCodePanelWidth.value = 0.68;
}

function refreshArticleEditorPreview() {
  articleEditorPreviewRefreshKey.value += 1;
}

function toggleArticleEditorFullscreen() {
  articleEditorFullscreen.value = !articleEditorFullscreen.value;
}

function getArticleEditorWorkspaceStyle() {
  const codePercent = `${Math.round(articleEditorCodePanelWidth.value * 100)}%`;
  return {
    gridTemplateColumns: `minmax(360px, ${codePercent}) 10px minmax(300px, 1fr)`
  };
}

function updateArticleEditorCursor(position?: { line: number; column: number }) {
  articleEditorCursor.line = position?.line ?? 1;
  articleEditorCursor.column = position?.column ?? 1;
}

function stopArticleEditorResize() {
  articleEditorResizeCleanup?.();
  articleEditorResizeCleanup = null;
}

function startArticleEditorResize(event: MouseEvent) {
  const workspace = articleEditorWorkspace.value;
  if (!workspace || event.button !== 0) return;

  const bounds = workspace.getBoundingClientRect();
  const minCodeWidth = 360;
  const minPreviewWidth = 300;

  const onPointerMove = (moveEvent: MouseEvent) => {
    const nextCodeWidth = Math.min(
      Math.max(moveEvent.clientX - bounds.left, minCodeWidth),
      bounds.width - minPreviewWidth - 10
    );
    articleEditorCodePanelWidth.value = nextCodeWidth / bounds.width;
  };

  const onPointerUp = () => {
    stopArticleEditorResize();
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  };

  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";

  window.addEventListener("mousemove", onPointerMove);
  window.addEventListener("mouseup", onPointerUp, { once: true });

  articleEditorResizeCleanup = () => {
    window.removeEventListener("mousemove", onPointerMove);
    window.removeEventListener("mouseup", onPointerUp);
  };
}

async function onArticleEditorWindowKeydown(event: KeyboardEvent) {
  if (!articleEditorOpen.value) return;
  if (event.defaultPrevented) return;

  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
    event.preventDefault();
    if (!articleEditorIsReadonly.value) {
      await saveArticleEditor();
    }
    return;
  }

  if (event.key === "F11") {
    event.preventDefault();
    toggleArticleEditorFullscreen();
    return;
  }

  if (event.key === "Escape") {
    event.preventDefault();
    closeArticleEditor();
  }
}

async function performDeleteArticle(relativePath: string) {
  try {
    articleStore.error = "";
    articleStore.lastMessage = "";
    await apiDelete(`/api/articles/${encodePathSegments(relativePath)}`);

    if (articleStore.current?.summary.relative_path === relativePath) {
      articleStore.current = null;
      articleEditorOpen.value = false;
      articleEditorDraft.value = "";
      articleEditorSelectedSourceKey.value = "original";
    }

    resetPreview(relativePath);
    articleStore.lastMessage = t("messages.articles.deleted");
    await articleStore.loadAll();
  } catch (error) {
    articleStore.error = error instanceof Error ? error.message : String(error);
  }
}

async function deleteArticle(relativePath: string) {
  const target = sortedArticles.value.find((article) => article.relative_path === relativePath);
  const title = target?.title || target?.name || relativePath;

  if (!window.confirm(t("articles.deletePrompt", { title }))) {
    return;
  }

  await performDeleteArticle(relativePath);
}

async function deleteSelectedArticles() {
  if (!selectedPaths.value.length) return;

  if (!window.confirm(t("articles.batchDeletePrompt", { count: selectedPaths.value.length }))) {
    return;
  }

  const deletingPaths = [...selectedPaths.value];
  for (const relativePath of deletingPaths) {
    await performDeleteArticle(relativePath);
  }

  selectedPaths.value = [];
  batchMode.value = false;
}

function onCardClick(article: ArticleSummary) {
  if (batchMode.value) {
    toggleSelected(article.relative_path);
    return;
  }

  void openArticleEditor(article.relative_path);
}

watch(
  filteredArticles,
  (articles) => {
    articles.forEach((article) => void ensurePreview(article.relative_path));
    const available = new Set(sortedArticles.value.map((item) => item.relative_path));
    selectedPaths.value = selectedPaths.value.filter((path) => available.has(path));
  },
  { immediate: true }
);

watch(
  () => articleEditorOpen.value,
  (open) => {
    if (open) {
      articleStore.lastMessage = "";
      articleStore.error = "";
      window.addEventListener("keydown", onArticleEditorWindowKeydown);
      nextTick(() => {
        articleMonacoEditor.value?.focus();
        updateArticleEditorCursor();
      });
      return;
    }

    stopArticleEditorResize();
    articleEditorFullscreen.value = false;
    window.removeEventListener("keydown", onArticleEditorWindowKeydown);
  }
);

watch(articleEditorSelectedSourceKey, () => {
  refreshArticleEditorPreview();
  nextTick(() => {
    articleMonacoEditor.value?.focus();
    updateArticleEditorCursor();
  });
});

if (!articleStore.articles.length && !articleStore.loading) {
  void articleStore.loadAll();
}

onBeforeUnmount(() => {
  stopArticleEditorResize();
  window.removeEventListener("keydown", onArticleEditorWindowKeydown);
});
</script>

<template>
  <section class="aiw-article-page">
    <div class="aiw-manager-layout">
      <aside class="aiw-manager-sidebar">
        <div class="aiw-sidebar-header">
          <h3>{{ t("articles.sidebarTitle") }}</h3>
        </div>

        <div class="aiw-sidebar-tree">
          <button
            v-for="status in statusOptions"
            :key="status.key"
            class="aiw-tree-item"
            :class="{ active: currentStatus === status.key }"
            type="button"
            @click="currentStatus = status.key"
          >
            <div class="aiw-tree-item-main">
              <span class="aiw-tree-icon" aria-hidden="true">
                <svg v-if="status.key === 'all'" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
                  <polyline points="14,2 14,8 20,8" />
                </svg>
                <svg v-else-if="status.key === 'published'" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                  <polyline points="22 4 12 14.01 9 11.01" />
                </svg>
                <svg v-else-if="status.key === 'failed'" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10" />
                  <line x1="15" y1="9" x2="9" y2="15" />
                  <line x1="9" y1="9" x2="15" y2="15" />
                </svg>
                <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10" />
                  <line x1="8" y1="12" x2="16" y2="12" />
                </svg>
              </span>
              <span class="aiw-tree-name">{{ status.label }}</span>
            </div>
            <span class="aiw-item-count">{{ status.count }}</span>
          </button>
        </div>
      </aside>

      <main class="aiw-manager-main-wrapper">
        <div class="aiw-manager-toolbar">
          <label class="aiw-search-box">
            <svg class="aiw-search-icon" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8" />
              <path d="m21 21-4.35-4.35" />
            </svg>
            <input v-model="searchKeyword" type="search" :placeholder="t('articles.searchPlaceholder')" />
          </label>

          <div class="aiw-toolbar-actions">
            <div class="aiw-view-toggle">
              <button class="aiw-view-btn" :class="{ active: layout === 'grid' }" type="button" :title="t('articles.gridView')" @click="layout = 'grid'">
                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="3" y="3" width="7" height="7" />
                  <rect x="14" y="3" width="7" height="7" />
                  <rect x="3" y="14" width="7" height="7" />
                  <rect x="14" y="14" width="7" height="7" />
                </svg>
              </button>
              <button class="aiw-view-btn" :class="{ active: layout === 'list' }" type="button" :title="t('articles.listView')" @click="layout = 'list'">
                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="8" y1="6" x2="21" y2="6" />
                  <line x1="8" y1="12" x2="21" y2="12" />
                  <line x1="8" y1="18" x2="21" y2="18" />
                  <line x1="3" y1="6" x2="3.01" y2="6" />
                  <line x1="3" y1="12" x2="3.01" y2="12" />
                  <line x1="3" y1="18" x2="3.01" y2="18" />
                </svg>
              </button>
            </div>

            <div class="aiw-batch-actions-group">
              <button class="btn btn-secondary aiw-batch-toggle" type="button" @click="toggleBatchMode">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M9 11l3 3L22 4" />
                  <path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" />
                </svg>
                <span>{{ t("articles.batchMode") }}</span>
                <span v-if="batchMode" class="aiw-batch-count">{{ t("articles.selectedCount", { count: selectedPaths.length }) }}</span>
              </button>

              <div v-if="batchMode" class="aiw-batch-sub-actions">
                <button class="btn btn-secondary" type="button" disabled :title="t('articles.publishUnavailable')">
                  <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M22 2L11 13" />
                    <path d="M22 2l-7 20-4-9-9-4 20-7z" />
                  </svg>
                  {{ t("articles.batchPublish") }}
                </button>
                <button class="btn btn-danger" type="button" :disabled="!selectedPaths.length" @click="deleteSelectedArticles">
                  <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                    <polyline points="3 6 5 6 21 6" />
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                  </svg>
                  {{ t("articles.batchDelete") }}
                </button>
              </div>
            </div>
          </div>
        </div>

        <div class="aiw-manager-main">
          <div class="aiw-content-grid" :class="{ 'list-view': layout === 'list' }">
            <div v-if="articleStore.loading" class="aiw-empty-state">{{ t("common.loading") }}</div>
            <div v-else-if="!filteredArticles.length" class="aiw-empty-state">{{ t("articles.emptyState") }}</div>

            <article
              v-for="article in filteredArticles"
              v-else
              :key="article.relative_path"
              class="aiw-content-card aiw-article-card"
              :class="{ 'batch-mode': batchMode }"
              @click="onCardClick(article)"
            >
              <label v-if="batchMode" class="aiw-checkbox-wrapper" @click.stop>
                <input class="aiw-batch-checkbox" type="checkbox" :checked="isSelected(article.relative_path)" @change="toggleSelected(article.relative_path)" />
                <span class="aiw-checkbox-custom"></span>
              </label>

              <div class="aiw-card-preview">
                <iframe
                  v-if="previewHtmlMap[article.relative_path]"
                  sandbox="allow-same-origin allow-scripts"
                  :srcdoc="previewHtmlMap[article.relative_path]"
                ></iframe>
                <div class="aiw-preview-loading">
                  {{ previewStateMap[article.relative_path] === "error" ? t("templates.previewLoadFailed") : t("common.loading") }}
                </div>
              </div>

              <div class="aiw-card-content">
                <h4 class="aiw-card-title" :title="article.title || article.name">{{ article.title || article.name }}</h4>
                <div class="aiw-card-meta">
                  <span class="aiw-format-badge">{{ article.format }}</span>
                  <span class="aiw-meta-divider">•</span>
                  <span class="aiw-status-badge" :class="statusClass(article.status)">{{ statusText(article.status) }}</span>
                  <span class="aiw-meta-divider">•</span>
                  <span>{{ formatBytes(article.size_bytes) }}</span>
                  <span class="aiw-meta-divider">•</span>
                  <span>{{ formatRelativeTime(article.updated_at) }}</span>
                  <span v-if="article.variant_count" class="aiw-meta-divider">/</span>
                  <span v-if="article.variant_count">{{ t("articles.variantCount", { count: article.variant_count }) }}</span>
                </div>
              </div>

              <div class="aiw-card-actions">
                <button class="aiw-card-action-btn" type="button" :title="t('articles.openEditor')" @click.stop="openArticleEditor(article.relative_path)">
                  <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8">
                    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                  </svg>
                </button>
                <button class="aiw-card-action-btn" type="button" :title="t('articles.previewTitle')" @click.stop="openArticleEditor(article.relative_path)">
                  <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8">
                    <rect x="3" y="3" width="7" height="7" />
                    <rect x="14" y="3" width="7" height="7" />
                    <rect x="3" y="14" width="7" height="7" />
                    <rect x="14" y="14" width="7" height="7" />
                    <path d="M10 10l4 4" />
                  </svg>
                </button>
                <button class="aiw-card-action-btn" type="button" disabled :title="t('articles.publishUnavailable')">
                  <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8">
                    <path d="M22 2L11 13" />
                    <path d="M22 2l-7 20-4-9-9-4 20-7z" />
                  </svg>
                </button>
                <button class="aiw-card-action-btn" type="button" :title="t('common.delete')" @click.stop="deleteArticle(article.relative_path)">
                  <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8">
                    <polyline points="3 6 5 6 21 6" />
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                  </svg>
                </button>
              </div>
            </article>
          </div>
        </div>
      </main>
    </div>

    <div
      v-if="articleEditorOpen && articleStore.current"
      class="config-modal-backdrop aiw-article-editor-backdrop"
      :class="{ 'aiw-article-editor-backdrop--fullscreen': articleEditorFullscreen }"
      @click.self="closeArticleEditor"
    >
      <div class="config-modal aiw-article-editor-modal" :class="{ 'aiw-article-editor-modal--fullscreen': articleEditorFullscreen }">
        <div class="aiw-article-editor-header">
          <div class="aiw-article-editor-title-group">
            <p class="eyebrow">{{ t("articles.editorEyebrow") }}</p>
            <h3 class="panel-title">{{ articleStore.current.summary.title || articleStore.current.summary.name }}</h3>
            <p class="aiw-article-editor-subtitle">{{ articleStore.current.summary.relative_path }}</p>
          </div>

          <div class="aiw-article-editor-actions">
            <select
              v-model="articleEditorSelectedSourceKey"
              class="text-input aiw-article-editor-source-select"
              :aria-label="t('articles.sourceLabel')"
            >
              <option v-for="option in articleEditorSourceOptions" :key="option.key" :value="option.key">
                {{ option.label }}
              </option>
            </select>
            <button
              class="template-icon-btn aiw-article-editor-toolbar-btn"
              type="button"
              :title="t('templates.refreshPreview')"
              @click="refreshArticleEditorPreview"
            >
              <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 12a9 9 0 1 1-2.64-6.36" />
                <path d="M21 3v6h-6" />
              </svg>
            </button>
            <button
              class="template-icon-btn aiw-article-editor-toolbar-btn"
              type="button"
              :title="articleEditorFullscreen ? t('templates.exitFullscreen') : t('templates.fullscreen')"
              @click="toggleArticleEditorFullscreen"
            >
              <svg v-if="!articleEditorFullscreen" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M8 3H5a2 2 0 0 0-2 2v3" />
                <path d="M16 3h3a2 2 0 0 1 2 2v3" />
                <path d="M8 21H5a2 2 0 0 1-2-2v-3" />
                <path d="M16 21h3a2 2 0 0 0 2-2v-3" />
              </svg>
              <svg v-else viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M9 3H5a2 2 0 0 0-2 2v4" />
                <path d="M15 3h4a2 2 0 0 1 2 2v4" />
                <path d="M3 15v4a2 2 0 0 0 2 2h4" />
                <path d="M21 15v4a2 2 0 0 1-2 2h-4" />
                <path d="M8 8 3 3" />
                <path d="m16 8 5-5" />
                <path d="m8 16-5 5" />
                <path d="m16 16 5 5" />
              </svg>
            </button>
            <button class="btn btn-danger" type="button" @click="deleteArticle(articleStore.current.summary.relative_path)">
              {{ t("common.delete") }}
            </button>
            <button class="btn btn-secondary" type="button" @click="closeArticleEditor">
              {{ t("common.close") }}
            </button>
            <button
              class="btn btn-primary"
              type="button"
              :disabled="articleEditorIsReadonly || articleStore.saving"
              :title="articleEditorIsReadonly ? t('articles.variantReadonlyHint') : undefined"
              @click="saveArticleEditor"
            >
              {{ articleStore.saving ? t("common.saving") : t("common.save") }}
            </button>
          </div>
        </div>

        <div class="aiw-article-editor-meta">
          <span>{{ t("common.path") }}: {{ articleStore.current.summary.relative_path }}</span>
          <span>{{ t("articles.sourceLabel") }}: {{ articleEditorActiveSourceLabel }}</span>
          <span>{{ t("common.format") }}: {{ articleEditorActiveSource?.format || articleStore.current.summary.format }}</span>
          <span>{{ t("common.status") }}: {{ statusText(articleStore.current.summary.status) }}</span>
          <span>{{ t("articles.variantCount", { count: articleStore.current.variants.length }) }}</span>
          <span v-if="articleEditorActiveVariant">{{ articleEditorActiveVariant.summary.platform_type }}</span>
          <span v-if="articleEditorIsReadonly" class="aiw-article-editor-readonly">
            {{ t("articles.variantReadonlyHint") }}
          </span>
        </div>

        <div class="aiw-article-editor-panels-header">
          <div class="aiw-article-editor-panels-side">
            <span>{{ articleEditorActiveSourceLabel }}</span>
            <span class="aiw-article-editor-status">{{ t("templates.editorCursor", articleEditorCursor) }}</span>
          </div>
          <div class="aiw-article-editor-panels-divider"></div>
          <div class="aiw-article-editor-panels-side aiw-article-editor-panels-side--preview">
            <span>{{ t("templates.editorLivePreview") }}</span>
            <button
              class="template-icon-btn aiw-article-editor-toolbar-btn"
              type="button"
              :title="t('templates.refreshPreview')"
              @click="refreshArticleEditorPreview"
            >
              <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 12a9 9 0 1 1-2.64-6.36" />
                <path d="M21 3v6h-6" />
              </svg>
            </button>
          </div>
        </div>

        <div ref="articleEditorWorkspace" class="aiw-article-editor-workspace" :style="getArticleEditorWorkspaceStyle()">
          <div class="aiw-article-editor-code-panel">
            <TemplateMonacoEditor
              ref="articleMonacoEditor"
              v-model="articleEditorContent"
              :language="articleEditorLanguage"
              :aria-label="articleEditorActiveSourceLabel"
              :readonly="articleEditorIsReadonly"
              autofocus
              @cursor-change="updateArticleEditorCursor"
              @save-shortcut="saveArticleEditor"
            />
          </div>

          <div class="aiw-article-editor-resize-divider" @mousedown="startArticleEditorResize"></div>

          <div class="aiw-article-editor-preview-panel">
            <iframe
              :key="articleEditorPreviewRefreshKey"
              class="aiw-article-editor-preview-frame"
              sandbox="allow-same-origin allow-scripts"
              :srcdoc="articleEditorPreviewDocument"
              :title="t('articles.previewTitle')"
            ></iframe>
          </div>
        </div>
      </div>
    </div>

    <div v-if="articleStore.lastMessage" class="banner banner-success">{{ articleStore.lastMessage }}</div>
    <div v-if="articleStore.error" class="banner banner-error">{{ articleStore.error }}</div>
  </section>
</template>

<style scoped>
.aiw-article-page {
  --aiw-primary: #3165eb;
  --aiw-primary-soft: rgba(49, 101, 235, 0.12);
  --aiw-surface: #ffffff;
  --aiw-background: #f7faff;
  --aiw-border: #d7e2f2;
  --aiw-ink: #1e293b;
  --aiw-ink-soft: #64748b;
  --aiw-ink-faint: #94a3b8;
  min-height: calc(100vh - 210px);
}

.aiw-manager-layout {
  display: flex;
  gap: 0;
  min-height: calc(100vh - 210px);
  border: 1px solid var(--aiw-border);
  border-radius: 18px;
  background: var(--aiw-surface);
  overflow: hidden;
}

.aiw-manager-sidebar {
  width: 220px;
  flex-shrink: 0;
  padding: 16px;
  border-right: 1px solid var(--aiw-border);
  background: var(--aiw-surface);
  overflow: auto;
}

.aiw-sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--aiw-border);
}

.aiw-sidebar-header h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  color: var(--aiw-ink);
}

.aiw-sidebar-tree {
  display: grid;
  gap: 6px;
}

.aiw-tree-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 10px 12px;
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: var(--aiw-ink-soft);
  text-align: left;
  cursor: pointer;
  transition: all 0.2s ease;
}

.aiw-tree-item:hover {
  background: #f4f8ff;
  color: var(--aiw-ink);
}

.aiw-tree-item.active {
  background: var(--aiw-primary);
  color: #ffffff;
}

.aiw-tree-item-main {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.aiw-tree-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.aiw-item-count {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 22px;
  height: 22px;
  padding: 0 6px;
  border-radius: 999px;
  background: rgba(15, 23, 42, 0.08);
  font-size: 11px;
  font-weight: 700;
}

.aiw-tree-item.active .aiw-item-count {
  background: rgba(255, 255, 255, 0.22);
}

.aiw-manager-main-wrapper {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}

.aiw-manager-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 16px 20px;
  border-bottom: 1px solid var(--aiw-border);
  background: var(--aiw-surface);
}

.aiw-search-box {
  position: relative;
  flex: 1;
  max-width: 430px;
}

.aiw-search-box input {
  width: 100%;
  min-height: 44px;
  padding: 10px 14px 10px 42px;
  border: 1px solid var(--aiw-border);
  border-radius: 10px;
  background: #fbfdff;
  color: var(--aiw-ink);
}

.aiw-search-box input:focus {
  outline: none;
  border-color: var(--aiw-primary);
  box-shadow: 0 0 0 3px rgba(49, 101, 235, 0.12);
}

.aiw-search-icon {
  position: absolute;
  left: 14px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--aiw-ink-faint);
}

.aiw-toolbar-actions,
.aiw-batch-actions-group,
.aiw-batch-sub-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.aiw-view-toggle {
  display: inline-flex;
  gap: 4px;
  padding: 2px;
  border: 1px solid var(--aiw-border);
  border-radius: 12px;
  background: #fbfdff;
}

.aiw-view-btn {
  width: 48px;
  height: 44px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: var(--aiw-ink-faint);
}

.aiw-view-btn.active {
  background: var(--aiw-primary);
  color: #ffffff;
}

.aiw-batch-toggle {
  white-space: nowrap;
}

.aiw-batch-count {
  margin-left: 4px;
  font-size: 12px;
  color: inherit;
  opacity: 0.8;
}

.aiw-manager-main {
  flex: 1;
  min-height: 0;
  overflow: auto;
  background: var(--aiw-background);
}

.aiw-content-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
  gap: 20px;
  padding: 14px;
  align-content: start;
}

.aiw-content-grid.list-view {
  grid-template-columns: 1fr;
}

.aiw-content-card {
  position: relative;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--aiw-border);
  border-radius: 14px;
  background: var(--aiw-surface);
  cursor: pointer;
  transition:
    transform 0.18s ease,
    border-color 0.18s ease,
    box-shadow 0.18s ease;
}

.aiw-content-card:hover {
  transform: translateY(-2px);
  border-color: rgba(49, 101, 235, 0.4);
  box-shadow: 0 10px 28px rgba(15, 23, 42, 0.08);
}

.aiw-content-grid.list-view .aiw-content-card {
  flex-direction: row;
  align-items: center;
  min-height: 88px;
}

.aiw-card-preview {
  position: relative;
  height: 162px;
  background: #eef4ff;
  overflow: hidden;
}

.aiw-content-grid.list-view .aiw-card-preview {
  width: 112px;
  height: 76px;
  flex-shrink: 0;
}

.aiw-card-preview iframe {
  width: 200%;
  height: 200%;
  border: 0;
  transform: scale(0.5);
  transform-origin: top left;
  pointer-events: none;
}

.aiw-preview-loading {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  font-size: 12px;
  color: var(--aiw-ink-faint);
}

.aiw-card-preview iframe + .aiw-preview-loading {
  display: none;
}

.aiw-card-content {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 14px;
  min-width: 0;
}

.aiw-content-grid.list-view .aiw-card-content {
  flex: 1;
}

.aiw-card-title {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: var(--aiw-ink);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.aiw-card-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
  font-size: 12px;
  color: var(--aiw-ink-soft);
}

.aiw-format-badge,
.aiw-status-badge {
  display: inline-flex;
  align-items: center;
  height: 28px;
  padding: 0 10px;
  border-radius: 8px;
  font-size: 11px;
  font-weight: 700;
}

.aiw-format-badge {
  background: var(--aiw-primary);
  color: #ffffff;
}

.aiw-status-badge {
  background: #e2e8f0;
  color: #475569;
}

.aiw-status-badge.published {
  background: #dcfce7;
  color: #15803d;
}

.aiw-status-badge.failed {
  background: #fee2e2;
  color: #b91c1c;
}

.aiw-status-badge.unpublished {
  background: #e2e8f0;
  color: #475569;
}

.aiw-meta-divider {
  color: var(--aiw-ink-faint);
}

.aiw-card-actions {
  display: flex;
  gap: 4px;
  padding: 10px 12px;
  border-top: 1px solid var(--aiw-border);
}

.aiw-content-grid.list-view .aiw-card-actions {
  width: auto;
  padding: 0 10px;
  border-top: 0;
  border-left: 1px solid var(--aiw-border);
  flex-shrink: 0;
}

.aiw-card-action-btn {
  flex: 1;
  height: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--aiw-ink-soft);
}

.aiw-card-action-btn:hover:not(:disabled) {
  background: #f4f8ff;
  color: var(--aiw-ink);
}

.aiw-card-action-btn:disabled {
  opacity: 0.42;
  cursor: not-allowed;
}

.aiw-checkbox-wrapper {
  position: absolute;
  top: 12px;
  right: 12px;
  z-index: 3;
}

.aiw-batch-checkbox {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}

.aiw-checkbox-custom {
  width: 18px;
  height: 18px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 2px solid var(--aiw-border);
  border-radius: 4px;
  background: #ffffff;
}

.aiw-batch-checkbox:checked + .aiw-checkbox-custom {
  border-color: var(--aiw-primary);
  background: var(--aiw-primary);
}

.aiw-batch-checkbox:checked + .aiw-checkbox-custom::after {
  content: "✓";
  color: #ffffff;
  font-size: 12px;
  font-weight: 700;
}

.aiw-empty-state {
  grid-column: 1 / -1;
  padding: 72px 20px;
  color: var(--aiw-ink-faint);
  text-align: center;
}

.aiw-empty-state--compact {
  padding: 20px;
}

.aiw-article-editor-backdrop--fullscreen {
  padding: 0;
}

.aiw-article-editor-modal {
  width: min(1280px, 96vw);
  max-height: min(92vh, 1000px);
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow: hidden;
}

.aiw-article-editor-modal--fullscreen {
  width: 100vw;
  max-width: none;
  height: 100vh;
  max-height: none;
  border-radius: 0;
}

.aiw-article-editor-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 18px;
}

.aiw-article-editor-title-group {
  min-width: 0;
}

.aiw-article-editor-subtitle {
  margin: 6px 0 0;
  color: var(--aiw-ink-soft);
  font-size: 13px;
  word-break: break-all;
}

.aiw-article-editor-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 10px;
}

.aiw-article-editor-source-select {
  min-width: 260px;
}

.aiw-article-editor-toolbar-btn {
  width: 40px;
  height: 40px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--aiw-border);
  border-radius: 12px;
  background: #ffffff;
  color: var(--aiw-ink-soft);
}

.aiw-article-editor-toolbar-btn:hover {
  border-color: rgba(49, 101, 235, 0.32);
  color: var(--aiw-primary);
}

.aiw-article-editor-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  font-size: 13px;
  color: var(--aiw-ink-soft);
}

.aiw-article-editor-readonly {
  color: #b45309;
  font-weight: 600;
}

.aiw-article-editor-panels-header {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 10px minmax(0, 1fr);
  align-items: center;
  min-height: 48px;
  border: 1px solid var(--aiw-border);
  border-radius: 14px;
  background: #f8fbff;
  color: var(--aiw-ink-soft);
}

.aiw-article-editor-panels-side {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-width: 0;
  padding: 0 14px;
}

.aiw-article-editor-panels-side > span:first-child {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--aiw-ink);
  font-weight: 600;
}

.aiw-article-editor-panels-side--preview {
  justify-content: space-between;
}

.aiw-article-editor-status {
  flex-shrink: 0;
  font-size: 12px;
  color: var(--aiw-ink-soft);
}

.aiw-article-editor-panels-divider {
  position: relative;
  height: 100%;
}

.aiw-article-editor-panels-divider::before {
  content: "";
  position: absolute;
  left: 4px;
  top: 10px;
  bottom: 10px;
  width: 2px;
  border-radius: 999px;
  background: var(--aiw-border);
}

.aiw-article-editor-workspace {
  display: grid;
  min-height: min(70vh, 720px);
  border: 1px solid var(--aiw-border);
  border-radius: 16px;
  overflow: hidden;
  background: #ffffff;
}

.aiw-article-editor-code-panel,
.aiw-article-editor-preview-panel {
  min-width: 0;
  min-height: 0;
}

.aiw-article-editor-code-panel {
  background: #ffffff;
  overflow: hidden;
}

.aiw-article-editor-resize-divider {
  width: 10px;
  cursor: col-resize;
  background:
    linear-gradient(90deg, transparent 0, transparent 4px, var(--aiw-border) 4px, var(--aiw-border) 6px, transparent 6px, transparent 100%);
  transition: background 120ms ease;
}

.aiw-article-editor-resize-divider:hover {
  background:
    linear-gradient(90deg, transparent 0, transparent 4px, rgba(49, 101, 235, 0.28) 4px, rgba(49, 101, 235, 0.28) 6px, transparent 6px, transparent 100%);
}

.aiw-article-editor-preview-panel {
  background: #eef4ff;
  min-height: 0;
}

.aiw-article-editor-preview-frame {
  width: 100%;
  height: 100%;
  border: 0;
  background: #ffffff;
}

@media (max-width: 1100px) {
  .aiw-manager-layout {
    min-height: auto;
  }

  .aiw-content-grid {
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  }

  .aiw-article-editor-panels-header,
  .aiw-article-editor-workspace {
    grid-template-columns: 1fr;
  }

  .aiw-article-editor-panels-divider,
  .aiw-article-editor-resize-divider {
    display: none;
  }

  .aiw-article-editor-source-select {
    min-width: 220px;
  }
}

@media (max-width: 860px) {
  .aiw-manager-layout {
    flex-direction: column;
  }

  .aiw-manager-sidebar {
    width: 100%;
    border-right: 0;
    border-bottom: 1px solid var(--aiw-border);
  }

  .aiw-manager-toolbar {
    flex-direction: column;
    align-items: stretch;
  }

  .aiw-search-box {
    max-width: none;
  }

  .aiw-toolbar-actions,
  .aiw-batch-actions-group {
    flex-wrap: wrap;
  }

  .aiw-article-editor-header {
    flex-direction: column;
  }

  .aiw-article-editor-actions {
    width: 100%;
    justify-content: flex-start;
  }

  .aiw-article-editor-source-select {
    width: 100%;
  }
}
</style>
