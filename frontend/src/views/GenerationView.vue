<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useGenerationStore } from "../stores/generation";
import { useTemplateStore } from "../stores/templates";
import type { GenerationTaskStatus, TemplateSummary } from "../types/postpub";

const generationStore = useGenerationStore();
const templateStore = useTemplateStore();
const { t } = useI18n();

const referenceModeEnabled = ref(false);
const logPanelOpen = ref(false);
const referenceUrlsText = ref("");

const templateOptions = computed(() =>
  templateStore.templates.filter(
    (template: TemplateSummary) => template.category === generationStore.form.template_category
  )
);

const currentTask = computed(() => generationStore.current ?? generationStore.tasks[0] ?? null);
const isGenerating = computed(
  () =>
    generationStore.creating ||
    currentTask.value?.status === "Pending" ||
    currentTask.value?.status === "Running"
);

const orderedEvents = computed(() => [...(currentTask.value?.events ?? [])].reverse());
const referenceRatioPercent = computed({
  get: () => Math.round((generationStore.form.reference_ratio || 0) * 100),
  set: (value: number) => {
    generationStore.form.reference_ratio = Number(value) / 100;
  }
});

const progressWidth = computed(() => {
  const status = currentTask.value?.status;
  if (generationStore.creating || status === "Pending") return "18%";
  if (status === "Running") return "68%";
  if (status === "Succeeded") return "100%";
  if (status === "Failed") return "100%";
  return "0%";
});

const subtitleText = computed(() =>
  referenceModeEnabled.value ? t("generation.workshopSubtitleReference") : t("generation.workshopSubtitle")
);

const currentTaskStatusLabel = computed(() => {
  const status = currentTask.value?.status;
  return status ? t(`generation.taskStatus.${status}`) : t("generation.noTaskYet");
});

function syncReferenceUrls() {
  generationStore.form.reference_urls = referenceUrlsText.value
    .split(/\r?\n|\|/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function taskStatusLabel(status: GenerationTaskStatus) {
  return t(`generation.taskStatus.${status}`);
}

function toggleReferenceMode() {
  referenceModeEnabled.value = !referenceModeEnabled.value;
  if (referenceModeEnabled.value) {
    logPanelOpen.value = false;
    return;
  }

  generationStore.form.reference_urls = [];
  referenceUrlsText.value = "";
}

function toggleLogPanel() {
  logPanelOpen.value = !logPanelOpen.value;
  if (logPanelOpen.value) {
    referenceModeEnabled.value = false;
  }
}

async function submitTask() {
  syncReferenceUrls();

  if (!referenceModeEnabled.value) {
    generationStore.form.reference_urls = [];
  }

  await generationStore.createTask();
  logPanelOpen.value = true;
}

watch(
  () => generationStore.form.template_category,
  () => {
    if (!templateOptions.value.some((item) => item.name === generationStore.form.template_name)) {
      generationStore.form.template_name = templateOptions.value[0]?.name ?? "";
    }
  }
);

watch(
  () => currentTask.value?.status,
  (status) => {
    if (!status) return;
    if (status === "Pending" || status === "Running" || status === "Failed") {
      logPanelOpen.value = true;
    }
  }
);

onMounted(async () => {
  if (!generationStore.tasks.length && !generationStore.loading) {
    await generationStore.loadTasks();
  }

  if (!templateStore.categories.length && !templateStore.loading) {
    await templateStore.loadAll();
  }

  referenceUrlsText.value = generationStore.form.reference_urls.join("\n");

  if (!templateOptions.value.some((item) => item.name === generationStore.form.template_name)) {
    generationStore.form.template_name = templateOptions.value[0]?.name ?? "";
  }
});

onBeforeUnmount(() => {
  generationStore.dispose();
});
</script>

<template>
  <section class="aiw-workshop-page">
    <div class="aiw-workshop-shell">
      <div class="aiw-workshop-header">
        <div class="aiw-logo-container">
          <svg class="aiw-logo-icon" viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
            <defs>
              <linearGradient id="aiw-grad-core" x1="0%" y1="0%" x2="100%" y2="100%">
                <stop offset="0%" style="stop-color: #00e5ff; stop-opacity: 1" />
                <stop offset="100%" style="stop-color: #bd00ff; stop-opacity: 1" />
              </linearGradient>
            </defs>
            <circle class="aiw-orbit-path aiw-orbit-1" cx="32" cy="32" r="28" />
            <circle class="aiw-orbit-path aiw-orbit-2" cx="32" cy="32" r="20" />
            <path class="aiw-core-x" d="M24 24 L40 40 M40 24 L24 40" stroke="url(#aiw-grad-core)" stroke-width="6" stroke-linecap="round" />
            <circle cx="32" cy="32" r="4" fill="#ffffff">
              <animate attributeName="opacity" values="0.4;1;0.4" dur="2s" repeatCount="indefinite" />
            </circle>
          </svg>
        </div>

        <h1 class="aiw-workshop-title">AIWriteX</h1>

        <div class="aiw-workshop-subtitle">
          <span class="aiw-subtitle-text">{{ subtitleText }}</span>
          <span class="aiw-cursor-blink"></span>
        </div>
      </div>

      <div class="aiw-workshop-card">
        <textarea
          v-model="generationStore.form.topic"
          class="aiw-topic-input"
          :placeholder="t('generation.topicPlaceholder')"
          rows="4"
          @keydown.enter.exact.prevent="submitTask"
        ></textarea>

        <div v-if="isGenerating || currentTask" class="aiw-bottom-progress">
          <div class="aiw-progress-track">
            <div class="aiw-progress-bar" :class="{ 'aiw-progress-bar--failed': currentTask?.status === 'Failed' }" :style="{ width: progressWidth }"></div>
          </div>
        </div>

        <div class="aiw-input-toolbar">
          <div class="aiw-toolbar-left">
            <button class="aiw-toolbar-btn" :class="{ 'is-active': referenceModeEnabled }" type="button" @click="toggleReferenceMode">
              <svg class="aiw-panel-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
              </svg>
              <span>{{ t("generation.referenceMode") }}</span>
            </button>

            <button class="aiw-toolbar-btn" :class="{ 'is-active': logPanelOpen }" type="button" @click="toggleLogPanel">
              <svg class="aiw-panel-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 20h9" />
                <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
              </svg>
              <span>{{ t("generation.runLogs") }}</span>
            </button>
          </div>

          <button class="aiw-generate-btn" :class="{ 'is-generating': isGenerating }" type="button" :disabled="generationStore.creating" @click="submitTask">
            <svg class="aiw-panel-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z" />
            </svg>
            <span>{{ generationStore.creating ? t("common.submitting") : t("generation.startGenerate") }}</span>
          </button>
        </div>
      </div>

      <div class="aiw-config-panel" :class="{ 'is-collapsed': !referenceModeEnabled }">
        <div class="aiw-config-panel-header">
          <h3 class="aiw-config-panel-title">
            <svg class="aiw-panel-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
              <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
            </svg>
            {{ t("generation.parametersTitle") }}
          </h3>
        </div>

        <div class="aiw-config-panel-body">
          <div class="aiw-form-row">
            <label class="aiw-form-group">
              <span>{{ t("generation.fields.templateCategory") }}</span>
              <select v-model="generationStore.form.template_category" class="aiw-form-select">
                <option value="">{{ t("generation.randomCategory") }}</option>
                <option v-for="category in templateStore.categories" :key="category.name" :value="category.name">
                  {{ category.name }}
                </option>
              </select>
            </label>

            <label class="aiw-form-group">
              <span>{{ t("generation.fields.templateName") }}</span>
              <select v-model="generationStore.form.template_name" class="aiw-form-select">
                <option value="">{{ t("generation.randomTemplate") }}</option>
                <option v-for="template in templateOptions" :key="template.relative_path" :value="template.name">
                  {{ template.name }}
                </option>
              </select>
            </label>

            <label class="aiw-form-group">
              <span>{{ t("generation.fields.referenceRatio") }}</span>
              <select v-model="referenceRatioPercent" class="aiw-form-select">
                <option :value="10">10%</option>
                <option :value="20">20%</option>
                <option :value="30">30%</option>
                <option :value="50">50%</option>
                <option :value="75">75%</option>
              </select>
            </label>
          </div>

          <label class="aiw-form-group">
            <span>{{ t("generation.fields.referenceUrls") }}</span>
            <textarea
              v-model="referenceUrlsText"
              class="aiw-form-textarea"
              :placeholder="t('generation.referencePlaceholderDetailed')"
              rows="3"
              @change="syncReferenceUrls"
            ></textarea>
          </label>
        </div>
      </div>

      <div class="aiw-config-panel aiw-log-panel" :class="{ 'is-collapsed': !logPanelOpen }">
        <div class="aiw-config-panel-header">
          <h3 class="aiw-config-panel-title">
            <svg class="aiw-panel-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 20h9" />
              <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
            </svg>
            {{ t("generation.logsDetail") }}
          </h3>

          <div class="aiw-log-panel-actions">
            <span class="aiw-task-status-pill" :class="`is-${currentTask?.status?.toLowerCase() || 'idle'}`">
              {{ currentTaskStatusLabel }}
            </span>
            <button class="aiw-icon-btn" type="button" @click="logPanelOpen = false">
              <svg class="aiw-panel-icon aiw-small-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M18 6 6 18" />
                <path d="m6 6 12 12" />
              </svg>
            </button>
          </div>
        </div>

        <div class="aiw-config-panel-body aiw-log-panel-body">
          <div v-if="currentTask" class="aiw-log-meta">
            <div class="aiw-log-meta-item">
              <span>{{ t("common.createdAt") }}</span>
              <strong>{{ currentTask.created_at }}</strong>
            </div>
            <div class="aiw-log-meta-item">
              <span>{{ t("common.updatedAt") }}</span>
              <strong>{{ currentTask.updated_at }}</strong>
            </div>
            <div class="aiw-log-meta-item">
              <span>{{ t("common.savedArticle") }}</span>
              <strong>{{ currentTask.output?.article?.relative_path || "-" }}</strong>
            </div>
          </div>

          <div v-if="orderedEvents.length" class="aiw-logs-output">
            <div v-for="event in orderedEvents" :key="`${event.timestamp}-${event.stage}-${event.message}`" class="aiw-log-entry">
              <div class="aiw-log-entry-header">
                <strong>{{ event.stage }}</strong>
                <span>{{ taskStatusLabel(event.status) }}</span>
                <time>{{ event.timestamp }}</time>
              </div>
              <p>{{ event.message }}</p>
            </div>
          </div>

          <div v-else-if="generationStore.error" class="aiw-logs-empty">
            {{ generationStore.error }}
          </div>

          <div v-else class="aiw-logs-empty">
            {{ t("generation.logsEmpty") }}
          </div>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.aiw-workshop-page {
  --aiw-primary: #3165eb;
  --aiw-primary-hover: #2957d1;
  --aiw-primary-soft: rgba(49, 101, 235, 0.1);
  --aiw-surface: #ffffff;
  --aiw-surface-soft: #f8fbff;
  --aiw-background: #f7faff;
  --aiw-border: #d7e2f2;
  --aiw-ink: #1e293b;
  --aiw-ink-soft: #64748b;
  --aiw-ink-faint: #94a3b8;
  --aiw-success: #22c55e;
  --aiw-danger: #ef4444;
  min-height: calc(100vh - 210px);
}

.aiw-workshop-shell {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  width: 100%;
  max-width: 980px;
  margin: 0 auto;
  padding: 22px 0 12px;
}

.aiw-workshop-header {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0;
  margin-bottom: 4px;
  text-align: center;
}

.aiw-logo-container {
  width: 80px;
  height: 80px;
  margin-bottom: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 24px;
  background: linear-gradient(145deg, #1a1f35, #252b46);
  border: 1px solid rgba(255, 255, 255, 0.1);
  box-shadow:
    0 10px 25px rgba(49, 101, 235, 0.25),
    inset 0 0 15px rgba(255, 255, 255, 0.05);
}

.aiw-logo-icon {
  width: 48px;
  height: 48px;
  filter: drop-shadow(0 0 8px rgba(0, 229, 255, 0.45));
}

.aiw-orbit-path {
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
  transform-origin: center;
}

.aiw-orbit-1 {
  stroke: #00e5ff;
  stroke-dasharray: 60;
  animation: aiw-spin-cw 4s linear infinite;
}

.aiw-orbit-2 {
  stroke: #bd00ff;
  stroke-dasharray: 40;
  opacity: 0.82;
  animation: aiw-spin-ccw 5s linear infinite;
}

.aiw-core-x {
  animation: aiw-breathe 2.5s infinite ease-in-out;
}

.aiw-workshop-title {
  margin: 0 0 12px;
  font-size: 32px;
  font-weight: 800;
  letter-spacing: -0.5px;
  background: linear-gradient(135deg, #2c3e50 0%, #2563eb 50%, #d946ef 100%);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}

.aiw-workshop-subtitle {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 24px;
  font-size: 16px;
  font-weight: 500;
  color: var(--aiw-ink-soft);
}

.aiw-cursor-blink {
  width: 2px;
  height: 16px;
  background: var(--aiw-primary);
  animation: aiw-blink 1s step-end infinite;
}

.aiw-workshop-card,
.aiw-config-panel {
  width: 100%;
  max-width: 800px;
  background: var(--aiw-surface);
  border: 1px solid var(--aiw-border);
  border-radius: 12px;
  box-shadow: 0 4px 24px rgba(15, 23, 42, 0.06);
  overflow: hidden;
}

.aiw-topic-input {
  width: 100%;
  height: 98px;
  padding: 16px 18px;
  border: 0;
  resize: none;
  background: transparent;
  color: var(--aiw-ink);
  font-size: 18px;
  line-height: 1.35;
  font-family: inherit;
}

.aiw-topic-input:focus {
  outline: none;
}

.aiw-topic-input::placeholder {
  color: rgba(100, 116, 139, 0.42);
}

.aiw-bottom-progress {
  margin-top: -1px;
}

.aiw-progress-track {
  height: 3px;
  width: 100%;
  background: #e8eef8;
  overflow: hidden;
}

.aiw-progress-bar {
  position: relative;
  height: 100%;
  border-radius: 999px;
  background: linear-gradient(90deg, var(--aiw-primary), #3b82f6);
  transition: width 280ms ease;
}

.aiw-progress-bar::after {
  content: "";
  position: absolute;
  inset: 0;
  background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.4), transparent);
  animation: aiw-shimmer 1.9s linear infinite;
}

.aiw-progress-bar--failed {
  background: linear-gradient(90deg, #ef4444, #f97316);
}

.aiw-input-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 20px;
  border-top: 1px solid var(--aiw-border);
  background: var(--aiw-surface);
}

.aiw-toolbar-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.aiw-toolbar-btn,
.aiw-generate-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 36px;
  padding: 0 14px;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 500;
  white-space: nowrap;
  transition: all 0.2s ease;
}

.aiw-toolbar-btn {
  border: 1px solid var(--aiw-border);
  background: transparent;
  color: var(--aiw-ink-soft);
}

.aiw-toolbar-btn:hover {
  border-color: var(--aiw-primary);
  background: var(--aiw-background);
  color: var(--aiw-ink);
}

.aiw-toolbar-btn.is-active {
  border-color: var(--aiw-primary);
  background: var(--aiw-primary-soft);
  color: var(--aiw-primary);
}

.aiw-generate-btn {
  border: 0;
  background: var(--aiw-primary);
  color: #ffffff;
}

.aiw-generate-btn:hover:not(:disabled) {
  background: var(--aiw-primary-hover);
  box-shadow: 0 4px 12px rgba(49, 101, 235, 0.3);
  transform: translateY(-1px);
}

.aiw-generate-btn:disabled {
  opacity: 0.72;
  cursor: not-allowed;
}

.aiw-generate-btn.is-generating {
  background: var(--aiw-danger);
}

.aiw-panel-icon {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.aiw-config-panel {
  transition:
    max-height 0.24s ease,
    opacity 0.2s ease,
    margin 0.2s ease,
    border-color 0.2s ease;
}

.aiw-config-panel.is-collapsed {
  max-height: 0;
  opacity: 0;
  margin-top: -8px;
  border-color: transparent;
  box-shadow: none;
}

.aiw-config-panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 44px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--aiw-border);
  background: #fbfdff;
}

.aiw-config-panel-title {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--aiw-ink);
}

.aiw-config-panel-body {
  padding: 14px 16px 16px;
}

.aiw-form-row {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
  margin-bottom: 12px;
}

.aiw-form-group {
  display: grid;
  gap: 6px;
}

.aiw-form-group span {
  font-size: 13px;
  font-weight: 500;
  color: var(--aiw-ink);
}

.aiw-form-select,
.aiw-form-textarea {
  width: 100%;
  border: 1px solid var(--aiw-border);
  border-radius: 6px;
  background: #ffffff;
  color: var(--aiw-ink);
  font: inherit;
  transition:
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.aiw-form-select {
  min-height: 38px;
  padding: 8px 10px;
}

.aiw-form-textarea {
  min-height: 74px;
  padding: 10px 12px;
  resize: vertical;
}

.aiw-form-select:focus,
.aiw-form-textarea:focus {
  outline: none;
  border-color: var(--aiw-primary);
  box-shadow: 0 0 0 3px rgba(49, 101, 235, 0.12);
}

.aiw-log-panel {
  flex: 1;
  min-height: 220px;
}

.aiw-log-panel-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 220px;
}

.aiw-log-panel-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.aiw-task-status-pill {
  display: inline-flex;
  align-items: center;
  min-height: 24px;
  padding: 0 10px;
  border-radius: 999px;
  background: #e2e8f0;
  color: #475569;
  font-size: 12px;
  font-weight: 600;
}

.aiw-task-status-pill.is-pending {
  background: #dbeafe;
  color: #1d4ed8;
}

.aiw-task-status-pill.is-running {
  background: #dbeafe;
  color: #2563eb;
}

.aiw-task-status-pill.is-succeeded {
  background: #dcfce7;
  color: #15803d;
}

.aiw-task-status-pill.is-failed {
  background: #fee2e2;
  color: #b91c1c;
}

.aiw-task-status-pill.is-idle {
  background: #e2e8f0;
  color: #64748b;
}

.aiw-icon-btn {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--aiw-ink-soft);
}

.aiw-icon-btn:hover {
  border-color: var(--aiw-border);
  background: var(--aiw-background);
  color: var(--aiw-ink);
}

.aiw-small-icon {
  width: 14px;
  height: 14px;
}

.aiw-log-meta {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

.aiw-log-meta-item {
  display: grid;
  gap: 4px;
  padding: 10px 12px;
  border-radius: 10px;
  border: 1px solid var(--aiw-border);
  background: #fbfdff;
}

.aiw-log-meta-item span {
  font-size: 12px;
  color: var(--aiw-ink-soft);
}

.aiw-log-meta-item strong {
  font-size: 12px;
  color: var(--aiw-ink);
  word-break: break-all;
}

.aiw-logs-output {
  display: grid;
  gap: 10px;
  max-height: 340px;
  overflow: auto;
  padding-right: 4px;
}

.aiw-log-entry {
  padding: 12px 14px;
  border-radius: 10px;
  border: 1px solid var(--aiw-border);
  background: #ffffff;
}

.aiw-log-entry-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 6px;
  font-size: 12px;
}

.aiw-log-entry-header strong {
  color: var(--aiw-ink);
}

.aiw-log-entry-header span,
.aiw-log-entry-header time {
  color: var(--aiw-ink-soft);
}

.aiw-log-entry p {
  margin: 0;
  font-size: 13px;
  line-height: 1.6;
  color: var(--aiw-ink-soft);
  white-space: pre-wrap;
}

.aiw-logs-empty {
  display: grid;
  place-items: center;
  min-height: 180px;
  padding: 20px;
  color: var(--aiw-ink-faint);
  font-size: 14px;
  text-align: center;
}

@keyframes aiw-spin-cw {
  to {
    transform: rotate(360deg);
  }
}

@keyframes aiw-spin-ccw {
  to {
    transform: rotate(-360deg);
  }
}

@keyframes aiw-breathe {
  0%,
  100% {
    transform: scale(1);
    opacity: 1;
  }
  50% {
    transform: scale(0.9);
    opacity: 0.82;
  }
}

@keyframes aiw-blink {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0;
  }
}

@keyframes aiw-shimmer {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(100%);
  }
}

@media (max-width: 900px) {
  .aiw-workshop-page {
    min-height: auto;
  }

  .aiw-workshop-shell {
    padding-top: 8px;
  }

  .aiw-form-row,
  .aiw-log-meta {
    grid-template-columns: 1fr;
  }

  .aiw-input-toolbar {
    flex-direction: column;
    align-items: stretch;
  }

  .aiw-toolbar-left {
    width: 100%;
    flex-wrap: wrap;
  }

  .aiw-toolbar-btn,
  .aiw-generate-btn {
    justify-content: center;
  }
}
</style>
