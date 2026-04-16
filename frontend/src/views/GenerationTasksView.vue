<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  Button as AButton,
  Card as ACard,
  Empty as AEmpty,
  Input as AInput,
  Modal as AModal,
  Space as ASpace,
  Table as ATable,
  Tag as ATag,
  message,
} from "ant-design-vue";
import {
  DeleteOutlined,
  EyeOutlined,
  ReloadOutlined,
  SearchOutlined,
} from "@ant-design/icons-vue";
import { useI18n } from "vue-i18n";
import { useGenerationStore } from "../stores/generation";
import type {
  GenerationEvent,
  GenerationTaskStatus,
  GenerationTaskSummary,
} from "../types/postpub";

const generationStore = useGenerationStore();
const { t } = useI18n();

const searchKeyword = ref("");
const detailOpen = ref(false);
const selectedTaskId = ref("");
const selectedTaskIds = ref<string[]>([]);
const deletingTaskIds = ref<string[]>([]);
const batchDeleting = ref(false);
const retryingTaskId = ref("");
const tablePageSize = ref(20);

const detailTask = computed(
  () =>
    generationStore.tasks.find((task) => task.id === selectedTaskId.value) ||
    (generationStore.current?.id === selectedTaskId.value
      ? generationStore.current
      : null),
);
const orderedEvents = computed(() =>
  [...(detailTask.value?.events ?? [])].reverse(),
);
type EventDisplayState = "pending" | "current" | "completed" | "failed";

type DisplayEvent = GenerationEvent & {
  displayState: EventDisplayState;
};

const displayEvents = computed<DisplayEvent[]>(() => {
  const taskStatus = detailTask.value?.status;
  return orderedEvents.value.map((event, index) => ({
    ...event,
    displayState: resolveEventDisplayState(event, index, taskStatus),
  }));
});
const currentEvent = computed(() => displayEvents.value[0] ?? null);
const sourceSummaries = computed(() => detailTask.value?.output?.sources ?? []);
const selectedTasks = computed(() =>
  generationStore.tasks.filter((task) => selectedTaskIds.value.includes(task.id)),
);
const deletableSelectedTasks = computed(() =>
  selectedTasks.value.filter(canDeleteTask),
);
const rowSelection = computed(() => ({
  selectedRowKeys: selectedTaskIds.value,
  onChange: (keys: Array<string | number>) => {
    selectedTaskIds.value = keys.map((key) => String(key));
  },
  getCheckboxProps: (record: GenerationTaskSummary) => ({
    disabled: !canDeleteTask(record),
  }),
}));
const tablePagination = computed(() => ({
  pageSize: tablePageSize.value,
  showSizeChanger: true,
  pageSizeOptions: ["20", "50", "100"],
  onShowSizeChange: (_current: number, size: number) => {
    tablePageSize.value = size;
  },
  onChange: (_page: number, size: number) => {
    tablePageSize.value = size;
  },
}));

function asTask(record: unknown) {
  return record as GenerationTaskSummary;
}

function canDeleteTask(task: GenerationTaskSummary) {
  return task.status === "Succeeded" || task.status === "Failed";
}

function taskTitle(task: GenerationTaskSummary) {
  return task.request.topic.trim() || t("common.untitledTask");
}

function statusLabel(status?: GenerationTaskStatus) {
  return status
    ? t(`generation.taskStatus.${status}`)
    : t("generation.noTaskYet");
}

function modeLabel(task: GenerationTaskSummary) {
  const mode =
    task.output?.mode ||
    (task.request.reference_urls.length ? "reference" : "topic");
  if (mode === "search" || mode === "reference" || mode === "topic") {
    return t(`generation.modes.${mode}`);
  }
  return mode;
}

function stageLabel(stage: string) {
  const knownStages = new Set([
    "bootstrap",
    "prepare",
    "provider",
    "retrieval",
    "draft",
    "variant",
    "save",
    "finalize",
    "done",
    "failed",
  ]);
  return knownStages.has(stage)
    ? t(`generation.stages.${stage}`)
    : t("generation.stages.unknown", { stage });
}

function resolveEventDisplayState(
  event: GenerationEvent,
  index: number,
  taskStatus?: GenerationTaskStatus,
): EventDisplayState {
  if (index === 0) {
    if (taskStatus === "Failed" || event.stage === "failed") return "failed";
    if (taskStatus === "Succeeded") return "completed";
    if (taskStatus === "Pending") return "pending";
    return "current";
  }

  if (event.status === "Failed" || event.stage === "failed") {
    return "failed";
  }

  return "completed";
}

function eventStateLabel(state: EventDisplayState) {
  return t(`generation.eventStatus.${state}`);
}

function eventStateColor(state: EventDisplayState) {
  switch (state) {
    case "completed":
      return "success";
    case "failed":
      return "error";
    case "pending":
      return "default";
    default:
      return "processing";
  }
}

function taskSearchText(task: GenerationTaskSummary) {
  return [
    taskTitle(task),
    task.id,
    statusLabel(task.status),
    modeLabel(task),
    task.output?.article?.relative_path || "",
  ]
    .join(" ")
    .toLowerCase();
}

const filteredTasks = computed(() => {
  const keyword = searchKeyword.value.trim().toLowerCase();
  if (!keyword) return generationStore.tasks;
  return generationStore.tasks.filter((task) =>
    taskSearchText(task).includes(keyword),
  );
});

const tableColumns = computed(() => [
  {
    title: t("generation.fields.topic"),
    key: "topic",
    dataIndex: "request.topic",
    ellipsis: true,
  },
  { title: t("common.status"), key: "status", width: 120 },
  { title: t("common.mode"), key: "mode", width: 120 },
  {
    title: t("common.updatedAt"),
    key: "updated_at",
    dataIndex: "updated_at",
    width: 220,
  },
  {
    title: t("common.savedArticle"),
    key: "article",
    width: 240,
    ellipsis: true,
  },
  {
    title: t("common.actions"),
    key: "actions",
    width: 260,
    fixed: "right" as const,
  },
]);

function openTaskDetail(task: GenerationTaskSummary) {
  selectedTaskId.value = task.id;
  detailOpen.value = true;
  generationStore.current = task;
  if (task.status === "Pending" || task.status === "Running") {
    generationStore.connectToEvents(task.id);
  }
}

function closeTaskDetail() {
  detailOpen.value = false;
  selectedTaskId.value = "";
}

async function refreshTasks() {
  await generationStore.loadTasks();
}

async function retryTask(task: GenerationTaskSummary) {
  retryingTaskId.value = task.id;
  const nextTask = await generationStore.retryTask(task);
  retryingTaskId.value = "";
  if (!nextTask) return;
  selectedTaskId.value = nextTask.id;
  detailOpen.value = true;
  message.success(t("generation.retryTaskSuccess"));
}

function isDeletingTask(taskId: string) {
  return deletingTaskIds.value.includes(taskId);
}

function markDeletingTasks(taskIds: string[]) {
  deletingTaskIds.value = [...new Set([...deletingTaskIds.value, ...taskIds])];
}

function unmarkDeletingTasks(taskIds: string[]) {
  const deleted = new Set(taskIds);
  deletingTaskIds.value = deletingTaskIds.value.filter((taskId) => !deleted.has(taskId));
}

function syncTaskSelectionAfterDelete(taskIds: string[]) {
  const deleted = new Set(taskIds);
  selectedTaskIds.value = selectedTaskIds.value.filter((taskId) => !deleted.has(taskId));
  if (selectedTaskId.value && deleted.has(selectedTaskId.value)) {
    closeTaskDetail();
  }
}

async function performDeleteTasks(tasks: GenerationTaskSummary[]) {
  const taskIds = tasks.map((task) => task.id);
  if (!taskIds.length) {
    return [];
  }

  markDeletingTasks(taskIds);
  if (taskIds.length > 1) {
    batchDeleting.value = true;
  }

  try {
    const deletedIds =
      taskIds.length === 1
        ? ((await generationStore.deleteTask(taskIds[0])) ? taskIds : [])
        : await generationStore.deleteTasks(taskIds);
    syncTaskSelectionAfterDelete(deletedIds);
    return deletedIds;
  } finally {
    unmarkDeletingTasks(taskIds);
    batchDeleting.value = false;
  }
}

function confirmDeleteTask(task: GenerationTaskSummary) {
  AModal.confirm({
    title: t("generation.deleteTaskTitle"),
    content: t("generation.deleteTaskPrompt", {
      title: taskTitle(task),
    }),
    okText: t("common.delete"),
    cancelText: t("common.cancel"),
    okButtonProps: { danger: true },
    onOk: async () => {
      const deletedIds = await performDeleteTasks([task]);
      if (deletedIds.length) {
        message.success(t("generation.deleteTaskSuccess"));
      }
    },
  });
}

function confirmDeleteSelectedTasks() {
  const tasks = deletableSelectedTasks.value;
  if (!tasks.length) {
    return;
  }

  AModal.confirm({
    title: t("generation.batchDeleteTitle"),
    content: t("generation.batchDeletePrompt", {
      count: tasks.length,
    }),
    okText: t("common.delete"),
    cancelText: t("common.cancel"),
    okButtonProps: { danger: true },
    onOk: async () => {
      const deletedIds = await performDeleteTasks(tasks);
      if (deletedIds.length) {
        message.success(
          t("generation.batchDeleteTaskSuccess", {
            count: deletedIds.length,
          }),
        );
      }
    },
  });
}

function taskRowProps(task: GenerationTaskSummary) {
  return {
    onClick: (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        target?.closest(".ant-table-selection-column") ||
        target?.closest("button") ||
        target?.closest("a")
      ) {
        return;
      }

      openTaskDetail(task);
    },
    style: "cursor: pointer;",
  };
}

onMounted(async () => {
  if (!generationStore.tasks.length && !generationStore.loading) {
    await generationStore.loadTasks();
  }
});

watch(
  () => generationStore.tasks.map((task) => task.id),
  (taskIds) => {
    const activeIds = new Set(taskIds);
    selectedTaskIds.value = selectedTaskIds.value.filter((taskId) =>
      activeIds.has(taskId),
    );
    if (selectedTaskId.value && !activeIds.has(selectedTaskId.value)) {
      closeTaskDetail();
    }
  },
);

onBeforeUnmount(() => {
  generationStore.dispose();
});
</script>

<template>
  <section class="task-status-page">
    <div v-if="generationStore.error" class="banner banner-error">
      {{ generationStore.error }}
    </div>

    <ACard :title="t('generation.taskListTitle')" class="task-status-card">
      <template #extra>
        <div class="task-toolbar">
          <AInput
            v-model:value="searchKeyword"
            :placeholder="t('generation.taskSearchPlaceholder')"
            allow-clear
            class="task-search-input"
          >
            <template #prefix>
              <SearchOutlined />
            </template>
          </AInput>
          <ASpace>
            <span
              v-if="selectedTaskIds.length"
              class="task-selection-hint"
            >
              {{
                t("generation.selectedCount", {
                  count: selectedTaskIds.length,
                })
              }}
            </span>
            <AButton
              danger
              :disabled="!deletableSelectedTasks.length"
              :loading="batchDeleting"
              @click="confirmDeleteSelectedTasks"
            >
              <template #icon>
                <DeleteOutlined />
              </template>
              {{ t("generation.batchDelete") }}
            </AButton>
          </ASpace>
        </div>
      </template>

      <ATable
        :columns="tableColumns"
        :data-source="filteredTasks"
        :pagination="tablePagination"
        :scroll="{ x: 1240 }"
        row-key="id"
        :row-selection="rowSelection"
        :custom-row="taskRowProps"
      >
        <template #emptyText>
          <AEmpty :description="t('generation.taskListEmpty')" />
        </template>

        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'topic'">
            <div class="task-table-topic">
              <strong>{{ taskTitle(asTask(record)) }}</strong>
              <small>{{ asTask(record).id }}</small>
            </div>
          </template>

          <template v-else-if="column.key === 'status'">
            <ATag
              class="task-table-tag"
              :color="
                asTask(record).status === 'Succeeded'
                  ? 'success'
                  : asTask(record).status === 'Failed'
                    ? 'error'
                    : 'processing'
              "
            >
              {{ statusLabel(asTask(record).status) }}
            </ATag>
          </template>

          <template v-else-if="column.key === 'mode'">
            {{ modeLabel(asTask(record)) }}
          </template>

          <template v-else-if="column.key === 'article'">
            <span>{{
              asTask(record).output?.article?.relative_path || "-"
            }}</span>
          </template>

          <template v-else-if="column.key === 'actions'">
            <ASpace>
              <AButton
                type="link"
                size="small"
                @click.stop="openTaskDetail(asTask(record))"
              >
                <template #icon>
                  <EyeOutlined />
                </template>
                {{ t("generation.viewDetails") }}
              </AButton>
              <AButton
                v-if="asTask(record).status === 'Failed'"
                danger
                size="small"
                :loading="retryingTaskId === asTask(record).id"
                @click.stop="retryTask(asTask(record))"
              >
                <template #icon>
                  <ReloadOutlined />
                </template>
                {{ t("generation.retryTask") }}
              </AButton>
              <AButton
                v-if="canDeleteTask(asTask(record))"
                danger
                size="small"
                :loading="isDeletingTask(asTask(record).id)"
                @click.stop="confirmDeleteTask(asTask(record))"
              >
                <template #icon>
                  <DeleteOutlined />
                </template>
                {{ t("common.delete") }}
              </AButton>
            </ASpace>
          </template>
        </template>
      </ATable>
    </ACard>

    <AModal
      v-model:open="detailOpen"
      :title="t('generation.taskDetailTitle')"
      width="1040px"
      destroy-on-close
      @cancel="closeTaskDetail"
    >
      <template v-if="detailTask">
        <div class="task-modal-sections">
          <ACard :title="t('generation.requestSummaryTitle')" size="small">
            <div class="info-grid">
              <div class="info-row">
                <span>{{ t("generation.fields.topic") }}</span>
                <strong>{{ taskTitle(detailTask) }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("common.status") }}</span>
                <strong>{{ statusLabel(detailTask.status) }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("common.mode") }}</span>
                <strong>{{ modeLabel(detailTask) }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("generation.fields.templateCategory") }}</span>
                <strong>{{
                  detailTask.request.template_category || "-"
                }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("generation.fields.templateName") }}</span>
                <strong>{{ detailTask.request.template_name || "-" }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("generation.referenceUrlCount") }}</span>
                <strong>{{ detailTask.request.reference_urls.length }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("common.createdAt") }}</span>
                <strong>{{ detailTask.created_at }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("common.updatedAt") }}</span>
                <strong>{{ detailTask.updated_at }}</strong>
              </div>
            </div>
          </ACard>

          <ACard :title="t('generation.outputSummaryTitle')" size="small">
            <div class="info-grid">
              <div class="info-row">
                <span>{{ t("common.format") }}</span>
                <strong>{{ detailTask.output?.format || "-" }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("common.savedArticle") }}</span>
                <strong>{{
                  detailTask.output?.article?.relative_path || "-"
                }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("generation.sourceCount") }}</span>
                <strong>{{ detailTask.output?.sources.length ?? 0 }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("generation.eventCount") }}</span>
                <strong>{{ detailTask.events.length }}</strong>
              </div>
            </div>
          </ACard>
        </div>

        <div
          v-if="detailTask.error"
          class="banner banner-error task-modal-banner"
        >
          {{ detailTask.error }}
        </div>

        <div class="task-modal-sections task-modal-sections--stacked">
          <ACard :title="t('generation.currentStepTitle')" size="small">
            <div v-if="currentEvent" class="task-current-step">
              <div class="task-event-list__header">
                <strong>{{ stageLabel(currentEvent.stage) }}</strong>
                <ATag
                  class="task-table-tag"
                  :color="eventStateColor(currentEvent.displayState)"
                >
                  {{ eventStateLabel(currentEvent.displayState) }}
                </ATag>
              </div>
              <p class="task-current-step__message">{{ currentEvent.message }}</p>
              <small>{{ currentEvent.timestamp }}</small>
            </div>
            <AEmpty v-else :description="t('generation.currentStepEmpty')" />
          </ACard>

          <ACard :title="t('generation.latestEvents')" size="small">
            <div v-if="displayEvents.length" class="task-event-list">
              <article
                v-for="event in displayEvents"
                :key="`${event.timestamp}-${event.stage}-${event.message}`"
                class="event-card"
                :class="`event-card--${event.displayState}`"
              >
                <div class="task-event-list__header">
                  <strong>{{ stageLabel(event.stage) }}</strong>
                  <ATag
                    class="task-table-tag"
                    :color="eventStateColor(event.displayState)"
                  >
                    {{ eventStateLabel(event.displayState) }}
                  </ATag>
                </div>
                <small>{{ event.timestamp }}</small>
                <p>{{ event.message }}</p>
              </article>
            </div>
            <AEmpty v-else :description="t('generation.logsEmpty')" />
          </ACard>

          <ACard :title="t('generation.sourceSummaryTitle')" size="small">
            <div v-if="sourceSummaries.length" class="task-source-list">
              <article
                v-for="source in sourceSummaries"
                :key="source.url"
                class="inset-panel task-source-item"
              >
                <div class="task-source-item__header">
                  <strong>{{ source.title }}</strong>
                  <a :href="source.url" target="_blank" rel="noreferrer">{{
                    t("common.openSource")
                  }}</a>
                </div>
                <small>{{ source.published_at || "-" }}</small>
                <p>{{ source.abstract_text || source.content || "-" }}</p>
              </article>
            </div>
            <AEmpty v-else :description="t('generation.outputMissing')" />
          </ACard>
        </div>
      </template>

      <AEmpty v-else :description="t('generation.detailEmpty')" />

      <template #footer>
        <ASpace>
          <AButton @click="closeTaskDetail">{{ t("common.close") }}</AButton>
          <AButton
            v-if="detailTask && canDeleteTask(detailTask)"
            danger
            :loading="isDeletingTask(detailTask.id)"
            @click="confirmDeleteTask(detailTask)"
          >
            <template #icon>
              <DeleteOutlined />
            </template>
            {{ t("common.delete") }}
          </AButton>
          <AButton
            v-if="detailTask?.status === 'Failed'"
            danger
            :loading="retryingTaskId === detailTask.id"
            @click="retryTask(detailTask)"
          >
            <template #icon>
              <ReloadOutlined />
            </template>
            {{ t("generation.retryTask") }}
          </AButton>
        </ASpace>
      </template>
    </AModal>
  </section>
</template>

<style scoped>
.task-status-page {
  display: grid;
  gap: 18px;
}

.task-status-hero {
  align-items: center;
}

.task-status-link {
  text-decoration: none;
}

.task-status-card :deep(.ant-card-head) {
  align-items: center;
}

.task-search-input {
  width: min(420px, 100%);
}

.task-toolbar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 12px;
}

.task-selection-hint {
  color: var(--ink-faint);
  white-space: nowrap;
}

.task-table-topic {
  display: grid;
  gap: 4px;
}

.task-table-topic strong {
  word-break: break-word;
}

.task-table-topic small {
  color: var(--ink-faint);
}

.task-table-tag {
  margin-inline-end: 0;
}

.task-modal-sections {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
}

.task-modal-sections--stacked {
  grid-template-columns: 1fr;
  margin-top: 16px;
}

.task-modal-banner {
  margin-top: 16px;
}

.task-event-list,
.task-source-list {
  display: grid;
  gap: 12px;
}

.task-current-step {
  display: grid;
  gap: 10px;
}

.task-current-step__message {
  margin: 0;
  font-size: 1rem;
  color: var(--ink);
}

.task-event-list__header,
.task-source-item__header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
}

.event-card {
  border-left: 4px solid transparent;
  transition: border-color 0.2s ease, background-color 0.2s ease;
}

.event-card--current {
  border-left-color: var(--primary);
  background: color-mix(in srgb, var(--primary) 7%, white);
}

.event-card--completed {
  border-left-color: #52c41a;
}

.event-card--failed {
  border-left-color: #ff4d4f;
  background: color-mix(in srgb, #ff4d4f 6%, white);
}

.event-card--pending {
  border-left-color: #d9d9d9;
}

.task-source-item {
  display: grid;
  gap: 8px;
}

.task-source-item__header strong {
  word-break: break-word;
}

.task-source-item__header a {
  color: var(--primary);
  text-decoration: none;
  white-space: nowrap;
}

.task-source-item__header a:hover {
  text-decoration: underline;
}

@media (max-width: 960px) {
  .task-modal-sections {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 720px) {
  .task-status-hero {
    align-items: flex-start;
  }

  .task-search-input {
    width: 100%;
  }

  .task-toolbar {
    width: 100%;
    justify-content: stretch;
  }

  .task-source-item__header,
  .task-event-list__header {
    flex-direction: column;
  }
}
</style>
