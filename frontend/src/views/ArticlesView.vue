<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { marked } from "marked";
import { useI18n } from "vue-i18n";
import { apiDelete, apiGet, encodePathSegments, type ApiResponse } from "../api/client";
import { useArticleStore } from "../stores/articles";
import { useConfigStore } from "../stores/config";
import type { ArticleDocument, ArticleSummary } from "../types/postpub";
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

const previewHtmlMap = reactive<Record<string, string>>({});
const previewStateMap = reactive<Record<string, "idle" | "loading" | "ready" | "error">>({});

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

    return [article.title, article.name, article.relative_path, article.format]
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

const articleEditorPreviewDocument = computed(() => {
  if (!articleStore.current) return "";
  const title = articleStore.current.summary.title || articleStore.current.summary.name || t("articles.previewTitle");
  const source = buildArticlePreviewSource(articleEditorDraft.value, articleStore.current.summary.format);
  return decoratePreviewDocument(source, configStore.designSurface, title);
});

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
  articleEditorOpen.value = true;
}

function closeArticleEditor() {
  articleEditorOpen.value = false;
}

async function saveArticleEditor() {
  if (!articleStore.current) return;
  articleStore.current.content = articleEditorDraft.value;
  await articleStore.saveCurrent();
  if (!articleStore.error) {
    resetPreview(articleStore.current.summary.relative_path);
    void ensurePreview(articleStore.current.summary.relative_path);
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
    if (!open) return;
    articleStore.lastMessage = "";
    articleStore.error = "";
  }
);

if (!articleStore.articles.length && !articleStore.loading) {
  void articleStore.loadAll();
}
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

    <div v-if="articleEditorOpen && articleStore.current" class="config-modal-backdrop" @click.self="closeArticleEditor">
      <div class="config-modal aiw-article-editor-modal">
        <div class="aiw-article-editor-header">
          <div>
            <p class="eyebrow">{{ t("articles.editorEyebrow") }}</p>
            <h3 class="panel-title">{{ articleStore.current.summary.title || articleStore.current.summary.name }}</h3>
          </div>

          <div class="btn-row">
            <button class="btn btn-primary" type="button" @click="saveArticleEditor">
              {{ articleStore.saving ? t("common.saving") : t("common.save") }}
            </button>
            <button class="btn btn-danger" type="button" @click="deleteArticle(articleStore.current.summary.relative_path)">
              {{ t("common.delete") }}
            </button>
            <button class="btn btn-secondary" type="button" @click="closeArticleEditor">
              {{ t("common.close") }}
            </button>
          </div>
        </div>

        <div class="aiw-article-editor-meta">
          <span>{{ t("common.path") }}: {{ articleStore.current.summary.relative_path }}</span>
          <span>{{ t("common.format") }}: {{ articleStore.current.summary.format }}</span>
          <span>{{ t("common.status") }}: {{ statusText(articleStore.current.summary.status) }}</span>
        </div>

        <div class="aiw-article-editor-body">
          <textarea v-model="articleEditorDraft" class="aiw-article-editor-textarea" spellcheck="false"></textarea>
          <iframe class="aiw-article-editor-preview" :srcdoc="articleEditorPreviewDocument" :title="t('articles.previewTitle')"></iframe>
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

.aiw-article-editor-modal {
  width: min(1120px, 96vw);
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.aiw-article-editor-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}

.aiw-article-editor-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  font-size: 13px;
  color: var(--aiw-ink-soft);
}

.aiw-article-editor-body {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(320px, 420px);
  gap: 16px;
  min-height: 62vh;
}

.aiw-article-editor-textarea {
  width: 100%;
  min-height: 100%;
  border: 1px solid var(--aiw-border);
  border-radius: 12px;
  background: #fbfdff;
  color: var(--aiw-ink);
  padding: 16px;
  resize: none;
  font: inherit;
  line-height: 1.65;
}

.aiw-article-editor-textarea:focus {
  outline: none;
  border-color: var(--aiw-primary);
  box-shadow: 0 0 0 3px rgba(49, 101, 235, 0.12);
}

.aiw-article-editor-preview {
  width: 100%;
  min-height: 100%;
  border: 1px solid var(--aiw-border);
  border-radius: 12px;
  background: #ffffff;
}

@media (max-width: 1100px) {
  .aiw-manager-layout {
    min-height: auto;
  }

  .aiw-content-grid {
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  }

  .aiw-article-editor-body {
    grid-template-columns: 1fr;
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
}
</style>
