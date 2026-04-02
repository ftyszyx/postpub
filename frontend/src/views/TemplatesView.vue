<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { apiGet, encodePathSegments, type ApiResponse } from "../api/client";
import TemplateMonacoEditor from "../components/TemplateMonacoEditor.vue";
import { useTemplateStore } from "../stores/templates";
import type { TemplateDocument, TemplateSummary } from "../types/postpub";
import { buildTemplatePreviewHtml, type TemplatePreviewMode } from "../utils/template-preview";

type TemplateLayout = "grid" | "list";
type ConfirmAction = "delete-category" | "delete-template" | null;

const templateStore = useTemplateStore();
const { locale, t } = useI18n();

const layout = ref<TemplateLayout>("grid");
const searchKeyword = ref("");
const selectedTemplatePath = ref("");
const dragTemplatePath = ref("");
const dragTemplateCategory = ref("");
const dropCategoryName = ref("");

const previewHtmlMap = reactive<Record<string, string>>({});
const previewStateMap = reactive<Record<string, "idle" | "loading" | "ready" | "error">>({});

const categoryModal = reactive({ open: false, mode: "create" as "create" | "rename", currentName: "", nextName: "" });
const templateEditorModal = reactive({
  open: false,
  mode: "create" as "create" | "edit",
  relativePath: "",
  name: "",
  category: "",
  content: ""
});
const templateRenameModal = reactive({ open: false, relativePath: "", nextName: "" });
const templateCopyModal = reactive({ open: false, relativePath: "", targetCategory: "", nextName: "" });
const confirmDialog = reactive({
  open: false,
  action: null as ConfirmAction,
  categoryName: "",
  templatePath: "",
  templateName: ""
});
const templateMonacoEditor = ref<{ focus: () => void } | null>(null);
const templateEditorWorkspace = ref<HTMLElement | null>(null);
const templateEditorPreviewRefreshKey = ref(0);
const templateEditorPreviewMode = ref<TemplatePreviewMode>("html");
const templateEditorFullscreen = ref(false);
const templateEditorCursor = reactive({ line: 1, column: 1 });
const templateEditorLanguage = computed<"html" | "markdown" | "plaintext">(() =>
  templateEditorPreviewMode.value === "text" ? "plaintext" : templateEditorPreviewMode.value
);
const templateEditorCodePanelWidth = ref(0.68);

let templateEditorResizeCleanup: (() => void) | null = null;

const totalTemplateCount = computed(() => templateStore.categories.reduce((count, item) => count + item.template_count, 0));
const visibleTemplateCount = computed(() => filteredTemplates.value.length);
const filteredTemplates = computed(() => {
  const keyword = searchKeyword.value.trim().toLowerCase();
  const templates = [...templateStore.templates].sort((left, right) => right.updated_at.localeCompare(left.updated_at));
  if (!keyword) return templates;
  return templates.filter((template) =>
    [template.name, template.category, template.relative_path].some((value) => value.toLowerCase().includes(keyword))
  );
});
const canCreateTemplate = computed(() => Boolean(templateStore.selectedCategory));
const templateEditorRenderedHtml = computed(() => buildPreviewHtml(templateEditorModal.content, templateEditorPreviewMode.value));
function defaultTemplateContent() {
  return [
    "<!DOCTYPE html>",
    '<html lang="zh-CN">',
    "  <head>",
    '    <meta charset="UTF-8" />',
    '    <meta name="viewport" content="width=device-width, initial-scale=1.0" />',
    "    <title>{{title}}</title>",
    "  </head>",
    "  <body>",
    "    <main>",
    "      <h1>{{title}}</h1>",
    "      <p>{{summary}}</p>",
    "      {{content}}",
    "    </main>",
    "  </body>",
    "</html>"
  ].join("\n");
}

function buildPreviewSampleData() {
  return {
    title: t("templates.previewSampleTitle"),
    summary: t("templates.previewSampleSummary"),
    paragraphOne: t("templates.previewSampleParagraphOne"),
    paragraphTwo: t("templates.previewSampleParagraphTwo"),
    generatedAt: new Date().toLocaleString(locale.value),
    author: "postpub",
    platform: t("config.sections.publishPlatformTypeWechat")
  };
}

function buildPreviewHtml(content: string, mode: TemplatePreviewMode = "html") {
  const sampleData = buildPreviewSampleData();
  return buildTemplatePreviewHtml(content, {
    title: sampleData.title,
    summary: sampleData.summary,
    paragraphOne: sampleData.paragraphOne,
    paragraphTwo: sampleData.paragraphTwo,
    generatedAt: sampleData.generatedAt,
    author: sampleData.author,
    platform: sampleData.platform
  }, mode);
}

async function ensurePreview(relativePath: string) {
  if (previewHtmlMap[relativePath] || previewStateMap[relativePath] === "loading") return;
  previewStateMap[relativePath] = "loading";
  try {
    const response = await apiGet<ApiResponse<TemplateDocument>>(`/api/templates/${encodePathSegments(relativePath)}`);
    previewHtmlMap[relativePath] = buildPreviewHtml(response.data.content);
    previewStateMap[relativePath] = "ready";
  } catch {
    previewStateMap[relativePath] = "error";
  }
}

function resetPreview(relativePath: string) {
  delete previewHtmlMap[relativePath];
  delete previewStateMap[relativePath];
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

function selectTemplate(template: TemplateSummary) {
  selectedTemplatePath.value = template.relative_path;
}

async function selectCategory(category?: string) {
  await templateStore.loadAll(category);
}

function openCreateCategoryModal() {
  categoryModal.open = true;
  categoryModal.mode = "create";
  categoryModal.currentName = "";
  categoryModal.nextName = "";
}

function openRenameCategoryModal(name: string) {
  categoryModal.open = true;
  categoryModal.mode = "rename";
  categoryModal.currentName = name;
  categoryModal.nextName = name;
}

function closeCategoryModal() {
  categoryModal.open = false;
}

async function submitCategoryModal() {
  const nextName = categoryModal.nextName.trim();
  if (!nextName) return;
  if (categoryModal.mode === "create") {
    await templateStore.createCategory(nextName);
  } else {
    await templateStore.renameCategory(categoryModal.currentName, nextName);
  }
  if (!templateStore.error) closeCategoryModal();
}

function openCreateTemplateModal() {
  if (!templateStore.selectedCategory) return;
  templateEditorModal.open = true;
  templateEditorModal.mode = "create";
  templateEditorModal.relativePath = "";
  templateEditorModal.name = "";
  templateEditorModal.category = templateStore.selectedCategory;
  templateEditorModal.content = defaultTemplateContent();
  templateEditorPreviewMode.value = "html";
  templateEditorPreviewRefreshKey.value += 1;
  resetTemplateEditorViewport();
}

async function openEditTemplateModal(template: TemplateSummary) {
  await templateStore.open(template.relative_path);
  if (!templateStore.current) return;
  selectedTemplatePath.value = template.relative_path;
  templateEditorModal.open = true;
  templateEditorModal.mode = "edit";
  templateEditorModal.relativePath = template.relative_path;
  templateEditorModal.name = templateStore.current.name;
  templateEditorModal.category = templateStore.current.category;
  templateEditorModal.content = templateStore.current.content;
  templateEditorPreviewMode.value = "html";
  templateEditorPreviewRefreshKey.value += 1;
  resetTemplateEditorViewport();
}

function closeTemplateEditorModal() {
  templateEditorModal.open = false;
  templateEditorFullscreen.value = false;
}

async function submitTemplateEditorModal() {
  if (templateEditorModal.mode === "create") {
    const name = templateEditorModal.name.trim();
    const category = templateEditorModal.category.trim();
    if (!name || !category) return;
    await templateStore.createTemplate(name, category, templateEditorModal.content);
    if (!templateStore.error && templateStore.current) {
      selectedTemplatePath.value = templateStore.current.relative_path;
      resetPreview(templateStore.current.relative_path);
      void ensurePreview(templateStore.current.relative_path);
      closeTemplateEditorModal();
    }
    return;
  }

  await templateStore.open(templateEditorModal.relativePath);
  if (!templateStore.current) return;
  templateStore.current.content = templateEditorModal.content;
  await templateStore.saveCurrent();
  if (!templateStore.error) {
    selectedTemplatePath.value = templateEditorModal.relativePath;
    resetPreview(templateEditorModal.relativePath);
    void ensurePreview(templateEditorModal.relativePath);
    closeTemplateEditorModal();
  }
}

function resetTemplateEditorViewport() {
  templateEditorCursor.line = 1;
  templateEditorCursor.column = 1;
  templateEditorCodePanelWidth.value = 0.68;
}

function refreshTemplateEditorPreview() {
  templateEditorPreviewRefreshKey.value += 1;
}

function toggleTemplateEditorFullscreen() {
  templateEditorFullscreen.value = !templateEditorFullscreen.value;
}

function getTemplateEditorWorkspaceStyle() {
  const codePercent = `${Math.round(templateEditorCodePanelWidth.value * 100)}%`;
  return {
    gridTemplateColumns: `minmax(360px, ${codePercent}) 10px minmax(300px, 1fr)`
  };
}

function updateTemplateEditorCursor(position?: { line: number; column: number }) {
  templateEditorCursor.line = position?.line ?? 1;
  templateEditorCursor.column = position?.column ?? 1;
}

function stopTemplateEditorResize() {
  templateEditorResizeCleanup?.();
  templateEditorResizeCleanup = null;
}

function startTemplateEditorResize(event: MouseEvent) {
  const workspace = templateEditorWorkspace.value;
  if (!workspace || event.button !== 0) return;

  const bounds = workspace.getBoundingClientRect();
  const minCodeWidth = 360;
  const minPreviewWidth = 300;

  const onPointerMove = (moveEvent: MouseEvent) => {
    const nextCodeWidth = Math.min(
      Math.max(moveEvent.clientX - bounds.left, minCodeWidth),
      bounds.width - minPreviewWidth - 10
    );
    templateEditorCodePanelWidth.value = nextCodeWidth / bounds.width;
  };

  const onPointerUp = () => {
    stopTemplateEditorResize();
    document.body.style.cursor = "";
    document.body.style.userSelect = "";
  };

  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";

  window.addEventListener("mousemove", onPointerMove);
  window.addEventListener("mouseup", onPointerUp, { once: true });

  templateEditorResizeCleanup = () => {
    window.removeEventListener("mousemove", onPointerMove);
    window.removeEventListener("mouseup", onPointerUp);
  };
}

async function onTemplateEditorWindowKeydown(event: KeyboardEvent) {
  if (!templateEditorModal.open) return;
  if (event.defaultPrevented) return;
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
    event.preventDefault();
    await submitTemplateEditorModal();
    return;
  }

  if (event.key === "F11") {
    event.preventDefault();
    toggleTemplateEditorFullscreen();
    return;
  }

  if (event.key === "Escape") {
    event.preventDefault();
    closeTemplateEditorModal();
  }
}

watch(templateEditorPreviewMode, () => {
  templateEditorPreviewRefreshKey.value += 1;
});

watch(
  () => templateEditorModal.open,
  (isOpen) => {
    if (isOpen) {
      window.addEventListener("keydown", onTemplateEditorWindowKeydown);
      nextTick(() => {
        templateMonacoEditor.value?.focus();
        updateTemplateEditorCursor();
      });
      return;
    }

    window.removeEventListener("keydown", onTemplateEditorWindowKeydown);
  }
);

function openRenameTemplateModal(template: TemplateSummary) {
  templateRenameModal.open = true;
  templateRenameModal.relativePath = template.relative_path;
  templateRenameModal.nextName = template.name;
}

function closeRenameTemplateModal() {
  templateRenameModal.open = false;
}

async function submitRenameTemplateModal() {
  const nextName = templateRenameModal.nextName.trim();
  if (!nextName) return;
  const isSelected = selectedTemplatePath.value === templateRenameModal.relativePath;
  const previousPath = templateRenameModal.relativePath;
  await templateStore.renameTemplate(previousPath, nextName, {
    focusResult: isSelected,
    reloadCategory: templateStore.selectedCategory || undefined
  });
  if (!templateStore.error) {
    resetPreview(previousPath);
    if (templateStore.current?.relative_path && isSelected) {
      selectedTemplatePath.value = templateStore.current.relative_path;
      void ensurePreview(templateStore.current.relative_path);
    }
    closeRenameTemplateModal();
  }
}

function openCopyTemplateModal(template: TemplateSummary) {
  templateCopyModal.open = true;
  templateCopyModal.relativePath = template.relative_path;
  templateCopyModal.targetCategory = template.category;
  templateCopyModal.nextName = `${template.name}-copy`;
}

function closeCopyTemplateModal() {
  templateCopyModal.open = false;
}

async function submitCopyTemplateModal() {
  const nextName = templateCopyModal.nextName.trim();
  const targetCategory = templateCopyModal.targetCategory.trim();
  if (!nextName || !targetCategory) return;
  await templateStore.copyTemplate(templateCopyModal.relativePath, targetCategory, nextName, {
    reloadCategory: templateStore.selectedCategory || undefined
  });
  if (!templateStore.error) closeCopyTemplateModal();
}

function openDeleteCategoryDialog(name: string) {
  confirmDialog.open = true;
  confirmDialog.action = "delete-category";
  confirmDialog.categoryName = name;
  confirmDialog.templatePath = "";
  confirmDialog.templateName = "";
}

function openDeleteTemplateDialog(template: TemplateSummary) {
  confirmDialog.open = true;
  confirmDialog.action = "delete-template";
  confirmDialog.categoryName = "";
  confirmDialog.templatePath = template.relative_path;
  confirmDialog.templateName = template.name;
}

function closeConfirmDialog() {
  confirmDialog.open = false;
  confirmDialog.action = null;
}

async function submitConfirmDialog() {
  if (confirmDialog.action === "delete-category") {
    await templateStore.deleteCategory(confirmDialog.categoryName);
  }
  if (confirmDialog.action === "delete-template") {
    const deletingSelected = selectedTemplatePath.value === confirmDialog.templatePath;
    await templateStore.deleteTemplate(confirmDialog.templatePath, templateStore.selectedCategory || undefined);
    resetPreview(confirmDialog.templatePath);
    if (deletingSelected) selectedTemplatePath.value = "";
  }
  if (!templateStore.error) closeConfirmDialog();
}

function onDragStart(template: TemplateSummary) {
  dragTemplatePath.value = template.relative_path;
  dragTemplateCategory.value = template.category;
}

function onDragEnd() {
  dragTemplatePath.value = "";
  dragTemplateCategory.value = "";
  dropCategoryName.value = "";
}

function onDragEnterCategory(categoryName: string) {
  if (!dragTemplatePath.value || dragTemplateCategory.value === categoryName) return;
  dropCategoryName.value = categoryName;
}

async function onDropToCategory(categoryName: string) {
  if (!dragTemplatePath.value || dragTemplateCategory.value === categoryName) {
    onDragEnd();
    return;
  }
  const movingSelected = selectedTemplatePath.value === dragTemplatePath.value;
  const previousPath = dragTemplatePath.value;
  await templateStore.moveTemplate(previousPath, categoryName, {
    focusResult: movingSelected,
    reloadCategory: templateStore.selectedCategory || undefined
  });
  resetPreview(previousPath);
  if (!templateStore.error && templateStore.current?.relative_path && movingSelected) {
    selectedTemplatePath.value = templateStore.current.relative_path;
    void ensurePreview(templateStore.current.relative_path);
  }
  onDragEnd();
}

watch(
  filteredTemplates,
  (templates) => {
    templates.forEach((template) => void ensurePreview(template.relative_path));
    if (selectedTemplatePath.value && !templates.some((item) => item.relative_path === selectedTemplatePath.value)) {
      selectedTemplatePath.value = "";
    }
  },
  { immediate: true }
);

onMounted(() => {
  if (!templateStore.categories.length && !templateStore.loading) {
    void templateStore.loadAll();
  }
});

onBeforeUnmount(() => {
  stopTemplateEditorResize();
  window.removeEventListener("keydown", onTemplateEditorWindowKeydown);
});
</script>

<template>
  <section class="panel template-manager-page">
    <div class="template-manager-layout">
      <aside class="template-manager-sidebar">
        <div class="template-sidebar-header">
          <h3>{{ t("templates.categoriesTitle") }}</h3>
          <button class="template-round-icon" type="button" @click="openCreateCategoryModal">
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 5v14" />
              <path d="M5 12h14" />
            </svg>
          </button>
        </div>

        <div class="template-sidebar-tree">
          <div class="template-tree-item" :class="{ 'template-tree-item--active': !templateStore.selectedCategory }" @click="selectCategory()">
            <div class="template-tree-main">
              <span class="template-tree-icon">
                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2z" />
                </svg>
              </span>
              <span class="template-tree-name">{{ t("templates.allTemplates") }}</span>
              <span class="template-tree-count">{{ totalTemplateCount }}</span>
            </div>
          </div>

          <div
            v-for="category in templateStore.categories"
            :key="category.name"
            class="template-tree-item"
            :class="{ 'template-tree-item--active': templateStore.selectedCategory === category.name, 'template-tree-item--drop': dropCategoryName === category.name }"
            @dragenter.prevent="onDragEnterCategory(category.name)"
            @dragover.prevent
            @dragleave="dropCategoryName = ''"
            @drop.prevent="onDropToCategory(category.name)"
          >
            <div class="template-tree-main" @click="selectCategory(category.name)">
              <span class="template-tree-icon">
                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M3 7v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-6l-2-2H5a2 2 0 0 0-2 2z" />
                </svg>
              </span>
              <span class="template-tree-name">{{ category.name }}</span>
              <span class="template-tree-count">{{ category.template_count }}</span>
            </div>
            <div class="template-tree-actions">
              <button class="template-icon-btn" type="button" @click.stop="openRenameCategoryModal(category.name)">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8">
                  <path d="M12 20h9" />
                  <path d="M16.5 3.5a2.1 2.1 0 1 1 3 3L7 19l-4 1 1-4z" />
                </svg>
              </button>
              <button class="template-icon-btn" type="button" @click.stop="openDeleteCategoryDialog(category.name)">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8">
                  <path d="M3 6h18" />
                  <path d="M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2" />
                  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
                </svg>
              </button>
            </div>
          </div>
        </div>
      </aside>

      <main class="template-manager-main">
        <div class="template-manager-toolbar">
          <div class="template-toolbar-search">
            <label class="template-search-box">
              <svg class="template-search-icon" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="11" cy="11" r="8" />
                <path d="m21 21-4.35-4.35" />
              </svg>
              <input v-model="searchKeyword" type="search" :placeholder="t('templates.searchPlaceholder')" />
            </label>
            <p class="template-toolbar-summary">
              {{ t("templates.currentCategoryCount", { count: visibleTemplateCount }) }}
            </p>
          </div>

          <div class="template-toolbar-actions">
            <div class="template-view-toggle">
              <button class="template-view-btn" :class="{ 'template-view-btn--active': layout === 'grid' }" type="button" @click="layout = 'grid'">
                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
                  <rect x="3" y="3" width="7" height="7" />
                  <rect x="14" y="3" width="7" height="7" />
                  <rect x="3" y="14" width="7" height="7" />
                  <rect x="14" y="14" width="7" height="7" />
                </svg>
              </button>
              <button class="template-view-btn" :class="{ 'template-view-btn--active': layout === 'list' }" type="button" @click="layout = 'list'">
                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M8 6h13" />
                  <path d="M8 12h13" />
                  <path d="M8 18h13" />
                  <path d="M3 6h.01" />
                  <path d="M3 12h.01" />
                  <path d="M3 18h.01" />
                </svg>
              </button>
            </div>

            <button class="btn btn-primary template-create-btn" type="button" :disabled="!canCreateTemplate" @click="openCreateTemplateModal">
              <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 5v14" />
                <path d="M5 12h14" />
              </svg>
              {{ t("templates.addTemplate") }}
            </button>
          </div>
        </div>

        <div class="template-grid" :class="{ 'template-grid--list': layout === 'list' }">
          <div v-if="templateStore.loading" class="empty-state">{{ t("common.loading") }}</div>
          <div v-else-if="!filteredTemplates.length" class="empty-state">{{ searchKeyword ? t("templates.searchEmpty") : t("templates.emptyState") }}</div>

          <article
            v-for="template in filteredTemplates"
            v-else
            :key="template.relative_path"
            class="template-content-card"
            :class="{ 'template-content-card--active': selectedTemplatePath === template.relative_path }"
            draggable="true"
            @click="selectTemplate(template)"
            @dragstart="onDragStart(template)"
            @dragend="onDragEnd"
          >
            <div class="template-card-preview">
              <iframe
                v-if="previewHtmlMap[template.relative_path]"
                class="template-card-iframe"
                sandbox="allow-same-origin allow-scripts"
                :srcdoc="previewHtmlMap[template.relative_path]"
              ></iframe>
              <div class="template-preview-loading">
                {{ previewStateMap[template.relative_path] === "error" ? t("templates.previewLoadFailed") : t("common.loading") }}
              </div>
            </div>

            <div class="template-card-content">
              <h4 class="template-card-title" :title="template.name">{{ template.name }}</h4>
              <div class="template-card-meta">
                <span class="template-meta-badge">{{ template.category }}</span>
                <span class="template-meta-divider">•</span>
                <span>{{ formatBytes(template.size_bytes) }}</span>
                <span class="template-meta-divider">•</span>
                <span>{{ formatRelativeTime(template.updated_at) }}</span>
              </div>
            </div>

            <div class="template-card-actions">
              <button class="template-icon-btn" type="button" @click.stop="openEditTemplateModal(template)">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8">
                  <path d="M12 20h9" />
                  <path d="M16.5 3.5a2.1 2.1 0 1 1 3 3L7 19l-4 1 1-4z" />
                </svg>
              </button>
              <button class="template-icon-btn" type="button" @click.stop="openRenameTemplateModal(template)">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8">
                  <path d="M4 7h16" />
                  <path d="M4 12h10" />
                  <path d="M4 17h10" />
                  <path d="M20 17l-4-4 4-4" />
                </svg>
              </button>
              <button class="template-icon-btn" type="button" @click.stop="openCopyTemplateModal(template)">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8">
                  <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                  <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
              </button>
              <button class="template-icon-btn" type="button" @click.stop="openDeleteTemplateDialog(template)">
                <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8">
                  <path d="M3 6h18" />
                  <path d="M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2" />
                  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
                </svg>
              </button>
            </div>
          </article>
        </div>
      </main>
    </div>

    <div v-if="templateStore.lastMessage" class="banner banner-success">{{ templateStore.lastMessage }}</div>
    <div v-if="templateStore.error" class="banner banner-error">{{ templateStore.error }}</div>
  </section>

  <div v-if="categoryModal.open" class="config-modal-backdrop" @click.self="closeCategoryModal">
    <div class="config-modal">
      <div class="config-modal__header">
        <div>
          <p class="eyebrow">{{ t("templates.categoriesEyebrow") }}</p>
          <h3 class="panel-title">{{ categoryModal.mode === "create" ? t("templates.addCategory") : t("templates.renameCategory") }}</h3>
        </div>
        <button class="btn btn-secondary" type="button" @click="closeCategoryModal">{{ t("common.close") }}</button>
      </div>

      <label class="field">
        <span>{{ t("common.name") }}</span>
        <input v-model="categoryModal.nextName" class="text-input" type="text" />
      </label>

      <div class="config-actions-bar">
        <button class="btn btn-secondary" type="button" @click="closeCategoryModal">{{ t("common.cancel") }}</button>
        <button class="btn btn-primary" type="button" @click="submitCategoryModal">
          {{ categoryModal.mode === "create" ? t("common.create") : t("common.save") }}
        </button>
      </div>
    </div>
  </div>

  <div
    v-if="templateEditorModal.open"
    class="config-modal-backdrop template-editor-backdrop"
    :class="{ 'template-editor-backdrop--fullscreen': templateEditorFullscreen }"
    @click.self="closeTemplateEditorModal"
  >
    <div class="config-modal template-editor-modal" :class="{ 'template-editor-modal--fullscreen': templateEditorFullscreen }">
      <div class="template-editor-header">
        <div class="template-editor-title-group">
          <h3 class="template-editor-title">
            {{ templateEditorModal.mode === "create" ? t("templates.addTemplate") : `${t("templates.editTemplate")} - ${templateEditorModal.name}` }}
          </h3>
          <p class="template-editor-subtitle">{{ templateEditorModal.category }}</p>
        </div>

        <div class="template-editor-actions">
          <select
            v-model="templateEditorPreviewMode"
            class="text-input template-editor-language"
            :aria-label="t('common.format')"
          >
            <option value="html">{{ t("config.formats.html") }}</option>
            <option value="markdown">{{ t("config.formats.markdown") }}</option>
            <option value="text">{{ t("config.formats.text") }}</option>
          </select>
          <button class="template-icon-btn template-editor-toolbar-btn" type="button" :title="t('templates.refreshPreview')" @click="refreshTemplateEditorPreview">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 12a9 9 0 1 1-2.64-6.36" />
              <path d="M21 3v6h-6" />
            </svg>
          </button>
          <button
            class="template-icon-btn template-editor-toolbar-btn"
            type="button"
            :title="templateEditorFullscreen ? t('templates.exitFullscreen') : t('templates.fullscreen')"
            @click="toggleTemplateEditorFullscreen"
          >
            <svg v-if="!templateEditorFullscreen" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
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
          <button class="btn btn-secondary" type="button" @click="closeTemplateEditorModal">{{ t("common.close") }}</button>
          <button class="btn btn-primary" type="button" @click="submitTemplateEditorModal">
            {{ templateEditorModal.mode === "create" ? t("common.create") : t("templates.saveShortcut") }}
          </button>
        </div>
      </div>

      <div v-if="templateEditorModal.mode === 'create'" class="template-editor-meta">
        <label class="field">
          <span>{{ t("common.name") }}</span>
          <input v-model="templateEditorModal.name" class="text-input" type="text" />
        </label>

        <label class="field">
          <span>{{ t("config.fields.templateCategory") }}</span>
          <select v-model="templateEditorModal.category" class="text-input">
            <option v-for="category in templateStore.categories" :key="category.name" :value="category.name">
              {{ category.name }}
            </option>
          </select>
        </label>
      </div>

      <div class="template-editor-panels-header">
        <div class="template-editor-panels-side">
          <span>{{ t("templates.editorCodeTitle") }}</span>
          <span class="template-editor-status">{{ t("templates.editorCursor", templateEditorCursor) }}</span>
        </div>
        <div class="template-editor-panels-divider"></div>
        <div class="template-editor-panels-side template-editor-panels-side--preview">
          <span>{{ t("templates.editorLivePreview") }}</span>
          <button class="template-icon-btn template-editor-toolbar-btn" type="button" :title="t('templates.refreshPreview')" @click="refreshTemplateEditorPreview">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 12a9 9 0 1 1-2.64-6.36" />
              <path d="M21 3v6h-6" />
            </svg>
          </button>
        </div>
      </div>

      <div ref="templateEditorWorkspace" class="template-editor-workspace" :style="getTemplateEditorWorkspaceStyle()">
        <div class="template-editor-code-panel">
          <TemplateMonacoEditor
            ref="templateMonacoEditor"
            v-model="templateEditorModal.content"
            :language="templateEditorLanguage"
            :aria-label="t('templates.editorCodeTitle')"
            autofocus
            @cursor-change="updateTemplateEditorCursor"
            @save-shortcut="submitTemplateEditorModal"
          />
        </div>

        <div class="template-editor-resize-divider" @mousedown="startTemplateEditorResize"></div>

        <div class="template-editor-preview-panel">
          <iframe
            :key="templateEditorPreviewRefreshKey"
            class="template-editor-preview-frame"
            sandbox="allow-same-origin allow-scripts"
            :srcdoc="templateEditorRenderedHtml"
          ></iframe>
        </div>
      </div>
    </div>
  </div>

  <div v-if="templateRenameModal.open" class="config-modal-backdrop" @click.self="closeRenameTemplateModal">
    <div class="config-modal">
      <div class="config-modal__header">
        <div>
          <p class="eyebrow">{{ t("templates.templatesEyebrow") }}</p>
          <h3 class="panel-title">{{ t("templates.renameTemplate") }}</h3>
        </div>
        <button class="btn btn-secondary" type="button" @click="closeRenameTemplateModal">{{ t("common.close") }}</button>
      </div>

      <label class="field">
        <span>{{ t("common.name") }}</span>
        <input v-model="templateRenameModal.nextName" class="text-input" type="text" />
      </label>

      <div class="config-actions-bar">
        <button class="btn btn-secondary" type="button" @click="closeRenameTemplateModal">{{ t("common.cancel") }}</button>
        <button class="btn btn-primary" type="button" @click="submitRenameTemplateModal">{{ t("common.save") }}</button>
      </div>
    </div>
  </div>

  <div v-if="templateCopyModal.open" class="config-modal-backdrop" @click.self="closeCopyTemplateModal">
    <div class="config-modal">
      <div class="config-modal__header">
        <div>
          <p class="eyebrow">{{ t("templates.templatesEyebrow") }}</p>
          <h3 class="panel-title">{{ t("templates.copyTemplate") }}</h3>
        </div>
        <button class="btn btn-secondary" type="button" @click="closeCopyTemplateModal">{{ t("common.close") }}</button>
      </div>

      <div class="form-grid two-columns">
        <label class="field">
          <span>{{ t("common.name") }}</span>
          <input v-model="templateCopyModal.nextName" class="text-input" type="text" />
        </label>

        <label class="field">
          <span>{{ t("config.fields.templateCategory") }}</span>
          <select v-model="templateCopyModal.targetCategory" class="text-input">
            <option v-for="category in templateStore.categories" :key="category.name" :value="category.name">
              {{ category.name }}
            </option>
          </select>
        </label>
      </div>

      <div class="config-actions-bar">
        <button class="btn btn-secondary" type="button" @click="closeCopyTemplateModal">{{ t("common.cancel") }}</button>
        <button class="btn btn-primary" type="button" @click="submitCopyTemplateModal">{{ t("common.copy") }}</button>
      </div>
    </div>
  </div>

  <div v-if="confirmDialog.open" class="config-modal-backdrop" @click.self="closeConfirmDialog">
    <div class="config-modal">
      <div class="config-modal__header">
        <div>
          <p class="eyebrow">{{ t("templates.templatesEyebrow") }}</p>
          <h3 class="panel-title">{{ t("templates.confirmTitle") }}</h3>
        </div>
        <button class="btn btn-secondary" type="button" @click="closeConfirmDialog">{{ t("common.close") }}</button>
      </div>

      <p class="workspace-copy template-copy">
        {{
          confirmDialog.action === "delete-category"
            ? t("templates.deleteCategoryConfirm", { name: confirmDialog.categoryName })
            : t("templates.deleteTemplateConfirm", { name: confirmDialog.templateName })
        }}
      </p>

      <div class="config-actions-bar">
        <button class="btn btn-secondary" type="button" @click="closeConfirmDialog">{{ t("common.cancel") }}</button>
        <button class="btn btn-danger" type="button" @click="submitConfirmDialog">{{ t("common.delete") }}</button>
      </div>
    </div>
  </div>
</template>
