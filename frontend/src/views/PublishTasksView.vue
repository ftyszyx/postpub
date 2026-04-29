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
  StopOutlined,
} from "@ant-design/icons-vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { usePublishStore } from "../stores/publish";
import type {
  PublishEvent,
  PublishTaskStatus,
  PublishTaskSummary,
} from "../types/postpub";

type EventDisplayState = "pending" | "current" | "completed" | "failed" | "canceled";
type DisplayEvent = PublishEvent & {
  displayState: EventDisplayState;
};

const publishStore = usePublishStore();
const route = useRoute();
const { t, te } = useI18n();

const searchKeyword = ref("");
const detailOpen = ref(false);
const selectedTaskId = ref("");
const selectedTaskIds = ref<string[]>([]);
const deletingTaskIds = ref<string[]>([]);
const batchDeleting = ref(false);
const retryingTaskId = ref("");
const cancelingTaskId = ref("");
const tablePageSize = ref(20);
const wechatLoginPromptTaskId = ref("");
let wechatLoginPromptModal: { destroy: () => void } | null = null;

const detailTask = computed(
  () =>
    publishStore.tasks.find((task) => task.id === selectedTaskId.value) ||
    (publishStore.current?.id === selectedTaskId.value
      ? publishStore.current
      : null),
);
const orderedEvents = computed(() =>
  [...(detailTask.value?.events ?? [])].reverse(),
);
const displayEvents = computed<DisplayEvent[]>(() => {
  const taskStatus = detailTask.value?.status;
  return orderedEvents.value.map((event, index) => ({
    ...event,
    displayState: resolveEventDisplayState(event, index, taskStatus),
  }));
});
const currentEvent = computed(() => displayEvents.value[0] ?? null);
const selectedTasks = computed(() =>
  publishStore.tasks.filter((task) => selectedTaskIds.value.includes(task.id)),
);
const deletableSelectedTasks = computed(() =>
  selectedTasks.value.filter(canDeleteTask),
);
const rowSelection = computed(() => ({
  selectedRowKeys: selectedTaskIds.value,
  onChange: (keys: Array<string | number>) => {
    selectedTaskIds.value = keys.map((key) => String(key));
  },
  getCheckboxProps: (record: PublishTaskSummary) => ({
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
  return record as PublishTaskSummary;
}

function canDeleteTask(task: PublishTaskSummary) {
  return task.status === "Succeeded" || task.status === "Failed" || task.status === "Canceled";
}

function canCancelTask(task: PublishTaskSummary) {
  return task.status === "Pending" || task.status === "Running";
}

function taskTitle(task: PublishTaskSummary) {
  return (
    task.output?.article_title ||
    task.request.article_relative_path ||
    t("common.untitledTask")
  );
}

function targetLabel(task: PublishTaskSummary) {
  return task.output?.target_name || task.request.target_id;
}

function statusLabel(status?: PublishTaskStatus) {
  return status ? t(`publish.taskStatus.${status}`) : t("publish.noTaskYet");
}

function modeLabel(task: PublishTaskSummary) {
  const mode = (
    task.output?.mode ||
    task.request.mode ||
    "draft"
  ).toLowerCase();
  if (mode === "draft" || mode === "publish") {
    return t(`publish.modes.${mode}`);
  }
  return mode;
}

function stageLabel(stage: string) {
  const knownStages = new Set([
    "bootstrap",
    "prepare",
    "target",
    "variant",
    "platform",
    "finalize",
    "done",
    "failed",
  ]);
  if (knownStages.has(stage)) {
    return t(`publish.stages.${stage}`);
  }

  if (stage.includes(".")) {
    return formatScopedStageLabel(stage);
  }

  return t("publish.stages.unknown", { stage });
}

function formatScopedStageLabel(stage: string) {
  const segmentKeys: Record<string, string> = {
    wechat: "config.sections.publishPlatformTypeWechat",
    auth: "publish.wechatLoginPromptTitle",
    browser: "config.sections.browserEnvironmentEyebrow",
    settings: "config.sections.wechatSettingsSummary",
    prepare: "publish.stages.prepare",
    target: "publish.stages.target",
    save: "common.save",
    required: "publish.wechatLoginRequiredStage",
    waiting: "publish.wechatLoginWaitingStage",
    token_detected: "publish.wechatLoginDetectedStage",
    done: "publish.stages.done",
    mode: "common.mode",
    close: "common.close",
  };

  return stage
    .split(".")
    .filter(Boolean)
    .map((segment) => {
      const key = segmentKeys[segment];
      if (key && te(key)) {
        return t(key);
      }

      return segment.replace(/_/g, " ");
    })
    .join(" / ");
}

function resolveEventDisplayState(
  event: PublishEvent,
  index: number,
  taskStatus?: PublishTaskStatus,
): EventDisplayState {
  if (index === 0) {
    if (taskStatus === "Canceled" || event.stage === "canceled") return "canceled";
    if (taskStatus === "Failed" || event.stage === "failed") return "failed";
    if (taskStatus === "Succeeded") return "completed";
    if (taskStatus === "Pending") return "pending";
    return "current";
  }

  if (event.status === "Canceled" || event.stage === "canceled") {
    return "canceled";
  }

  if (event.status === "Failed" || event.stage === "failed") {
    return "failed";
  }

  return "completed";
}

function eventStateLabel(state: EventDisplayState) {
  return t(`publish.eventStatus.${state}`);
}

function eventStateColor(state: EventDisplayState) {
  switch (state) {
    case "completed":
      return "success";
    case "failed":
      return "error";
    case "pending":
      return "default";
    case "canceled":
      return "warning";
    default:
      return "processing";
  }
}

function taskSearchText(task: PublishTaskSummary) {
  return [
    taskTitle(task),
    task.id,
    targetLabel(task),
    task.request.article_relative_path,
    task.request.target_id,
    statusLabel(task.status),
    modeLabel(task),
  ]
    .join(" ")
    .toLowerCase();
}

const filteredTasks = computed(() => {
  const keyword = searchKeyword.value.trim().toLowerCase();
  if (!keyword) return publishStore.tasks;
  return publishStore.tasks.filter((task) =>
    taskSearchText(task).includes(keyword),
  );
});

const tableColumns = computed(() => [
  {
    title: t("publish.fields.article"),
    key: "article",
    dataIndex: "request.article_relative_path",
    ellipsis: true,
  },
  {
    title: t("publish.fields.target"),
    key: "target",
    width: 180,
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
    title: t("common.actions"),
    key: "actions",
    width: 260,
    fixed: "right" as const,
  },
]);

function openTaskDetail(task: PublishTaskSummary) {
  selectedTaskId.value = task.id;
  detailOpen.value = true;
  publishStore.current = task;
  if (task.status === "Pending" || task.status === "Running") {
    publishStore.connectToEvents(task.id);
  }
}

function closeTaskDetail() {
  detailOpen.value = false;
  selectedTaskId.value = "";
}

async function retryTask(task: PublishTaskSummary) {
  retryingTaskId.value = task.id;
  try {
    const nextTask = await publishStore.retryTask(task);
    if (!nextTask) return;
    selectedTaskId.value = nextTask.id;
    detailOpen.value = true;
    message.success(t("publish.retryTaskSuccess"));
  } finally {
    retryingTaskId.value = "";
  }
}

function isCancelingTask(taskId: string) {
  return cancelingTaskId.value === taskId;
}

async function cancelTask(task: PublishTaskSummary) {
  cancelingTaskId.value = task.id;
  try {
    const nextTask = await publishStore.cancelTask(task);
    if (!nextTask) return;
    selectedTaskId.value = nextTask.id;
    message.success(t("publish.cancelTaskSuccess"));
  } finally {
    cancelingTaskId.value = "";
  }
}

function confirmCancelTask(task: PublishTaskSummary) {
  AModal.confirm({
    title: t("publish.cancelTaskTitle"),
    content: t("publish.cancelTaskPrompt", {
      title: taskTitle(task),
    }),
    okText: t("publish.cancelTask"),
    cancelText: t("common.cancel"),
    okButtonProps: { danger: true },
    onOk: async () => {
      await cancelTask(task);
    },
  });
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

async function performDeleteTasks(tasks: PublishTaskSummary[]) {
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
        ? ((await publishStore.deleteTask(taskIds[0])) ? taskIds : [])
        : await publishStore.deleteTasks(taskIds);
    syncTaskSelectionAfterDelete(deletedIds);
    return deletedIds;
  } finally {
    unmarkDeletingTasks(taskIds);
    batchDeleting.value = false;
  }
}

function confirmDeleteTask(task: PublishTaskSummary) {
  AModal.confirm({
    title: t("publish.deleteTaskTitle"),
    content: t("publish.deleteTaskPrompt", {
      title: taskTitle(task),
    }),
    okText: t("common.delete"),
    cancelText: t("common.cancel"),
    okButtonProps: { danger: true },
    onOk: async () => {
      const deletedIds = await performDeleteTasks([task]);
      if (deletedIds.length) {
        message.success(t("publish.deleteTaskSuccess"));
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
    title: t("publish.batchDeleteTitle"),
    content: t("publish.batchDeletePrompt", {
      count: tasks.length,
    }),
    okText: t("common.delete"),
    cancelText: t("common.cancel"),
    okButtonProps: { danger: true },
    onOk: async () => {
      const deletedIds = await performDeleteTasks(tasks);
      if (deletedIds.length) {
        message.success(
          t("publish.batchDeleteTaskSuccess", {
            count: deletedIds.length,
          }),
        );
      }
    },
  });
}

function taskRowProps(task: PublishTaskSummary) {
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

function taskNeedsWechatLoginPrompt(task: PublishTaskSummary | null | undefined) {
  if (!task || (task.status !== "Pending" && task.status !== "Running")) {
    return false;
  }

  let loginRequired = false;
  for (const event of task.events) {
    if (event.stage === "wechat.auth.required") {
      loginRequired = true;
      continue;
    }

    if (
      event.stage === "wechat.auth.token_detected" ||
      event.stage === "wechat.editor.ready" ||
      event.status === "Succeeded" ||
      event.status === "Failed"
    ) {
      loginRequired = false;
    }
  }

  return loginRequired;
}

function closeWechatLoginPrompt() {
  wechatLoginPromptModal?.destroy();
  wechatLoginPromptModal = null;
  wechatLoginPromptTaskId.value = "";
}

function openWechatLoginPrompt(task: PublishTaskSummary) {
  if (wechatLoginPromptTaskId.value === task.id && wechatLoginPromptModal) {
    return;
  }

  closeWechatLoginPrompt();
  wechatLoginPromptTaskId.value = task.id;
  wechatLoginPromptModal = AModal.info({
    title: t("publish.wechatLoginPromptTitle"),
    content: t("publish.wechatLoginPromptBody", {
      target: targetLabel(task),
    }),
    okText: t("publish.wechatLoginPromptOk"),
    closable: true,
    maskClosable: true,
    onOk: () => {
      wechatLoginPromptModal = null;
    },
    onCancel: () => {
      wechatLoginPromptModal = null;
    },
  });
}

function syncWechatLoginPrompt(task: PublishTaskSummary | null | undefined) {
  if (taskNeedsWechatLoginPrompt(task)) {
    openWechatLoginPrompt(task!);
    return;
  }

  closeWechatLoginPrompt();
}

async function openTaskFromRoute() {
  if (route.query.kind !== "publish" || typeof route.query.task !== "string") {
    return;
  }

  selectedTaskId.value = route.query.task;
  await publishStore.selectTask(route.query.task);
  detailOpen.value = true;
}

onMounted(async () => {
  if (!publishStore.tasks.length && !publishStore.loading) {
    await publishStore.loadTasks();
  }
  await openTaskFromRoute();
});

watch(
  () => route.query.task,
  async () => {
    await openTaskFromRoute();
  },
);

watch(
  () => publishStore.tasks.map((task) => task.id),
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

watch(
  () => ({
    id: (detailTask.value ?? publishStore.current)?.id ?? "",
    status: (detailTask.value ?? publishStore.current)?.status ?? "",
    events: ((detailTask.value ?? publishStore.current)?.events ?? [])
      .map((event) => `${event.stage}:${event.status}`)
      .join("|"),
  }),
  () => {
    syncWechatLoginPrompt(detailTask.value ?? publishStore.current);
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  closeWechatLoginPrompt();
  publishStore.dispose();
});
</script>

<template>
  <section class="task-status-page">
    <div v-if="publishStore.error" class="banner banner-error">
      {{ publishStore.error }}
    </div>

    <ACard :title="t('publish.taskListTitle')" class="task-status-card">
      <template #extra>
        <div class="task-toolbar">
          <AInput
            v-model:value="searchKeyword"
            :placeholder="t('publish.taskSearchPlaceholder')"
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
                t("publish.selectedCount", {
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
              {{ t("publish.batchDelete") }}
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
          <AEmpty :description="t('publish.taskListEmpty')" />
        </template>

        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'article'">
            <div class="task-table-topic">
              <strong>{{ taskTitle(asTask(record)) }}</strong>
              <small>{{ asTask(record).request.article_relative_path }}</small>
            </div>
          </template>

          <template v-else-if="column.key === 'target'">
            <div class="task-table-topic">
              <strong>{{ targetLabel(asTask(record)) }}</strong>
              <small>{{ asTask(record).request.target_id }}</small>
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
                    : asTask(record).status === 'Canceled'
                      ? 'warning'
                    : 'processing'
              "
            >
              {{ statusLabel(asTask(record).status) }}
            </ATag>
          </template>

          <template v-else-if="column.key === 'mode'">
            {{ modeLabel(asTask(record)) }}
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
                {{ t("publish.viewDetails") }}
              </AButton>
              <AButton
                v-if="canCancelTask(asTask(record))"
                danger
                size="small"
                :loading="isCancelingTask(asTask(record).id)"
                @click.stop="confirmCancelTask(asTask(record))"
              >
                <template #icon>
                  <StopOutlined />
                </template>
                {{ t("publish.cancelTask") }}
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
                {{ t("publish.retryTask") }}
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
      :title="t('publish.taskDetailTitle')"
      width="1040px"
      destroy-on-close
      @cancel="closeTaskDetail"
    >
      <template v-if="detailTask">
        <div class="task-modal-sections">
          <ACard :title="t('publish.requestSummaryTitle')" size="small">
            <div class="info-grid">
              <div class="info-row">
                <span>{{ t("publish.fields.article") }}</span>
                <strong>{{ detailTask.request.article_relative_path }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("publish.fields.target") }}</span>
                <strong>{{ targetLabel(detailTask) }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("publish.fields.targetId") }}</span>
                <strong>{{ detailTask.request.target_id }}</strong>
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
                <span>{{ t("publish.eventCount") }}</span>
                <strong>{{ detailTask.events.length }}</strong>
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

          <ACard :title="t('publish.outputSummaryTitle')" size="small">
            <div class="info-grid">
              <div class="info-row">
                <span>{{ t("publish.fields.platform") }}</span>
                <strong>{{ detailTask.output?.platform_type || "-" }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("common.format") }}</span>
                <strong>{{ detailTask.output?.format || "-" }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("publish.fields.remoteUrl") }}</span>
                <strong>{{ detailTask.output?.remote_url || "-" }}</strong>
              </div>
              <div class="info-row">
                <span>{{ t("publish.outputState") }}</span>
                <strong>{{
                  detailTask.output
                    ? t("publish.outputReady")
                    : t("publish.outputMissing")
                }}</strong>
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
          <ACard :title="t('publish.currentStepTitle')" size="small">
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
              <p class="task-current-step__message">
                {{ currentEvent.message }}
              </p>
              <small>{{ currentEvent.timestamp }}</small>
            </div>
            <AEmpty v-else :description="t('publish.currentStepEmpty')" />
          </ACard>

          <ACard :title="t('publish.latestEvents')" size="small">
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
            <AEmpty v-else :description="t('publish.logsEmpty')" />
          </ACard>
        </div>
      </template>

      <AEmpty v-else :description="t('publish.detailEmpty')" />

      <template #footer>
        <ASpace>
          <AButton @click="closeTaskDetail">{{ t("common.close") }}</AButton>
          <AButton
            v-if="detailTask && canCancelTask(detailTask)"
            danger
            :loading="isCancelingTask(detailTask.id)"
            @click="confirmCancelTask(detailTask)"
          >
            <template #icon>
              <StopOutlined />
            </template>
            {{ t("publish.cancelTask") }}
          </AButton>
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
            {{ t("publish.retryTask") }}
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

.task-event-list {
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

.task-event-list__header {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
}

.event-card {
  border-left: 4px solid transparent;
  transition:
    border-color 0.2s ease,
    background-color 0.2s ease;
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

@media (max-width: 960px) {
  .task-modal-sections {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 720px) {
  .task-search-input {
    width: 100%;
  }

  .task-toolbar {
    width: 100%;
    justify-content: stretch;
  }

  .task-event-list__header {
    flex-direction: column;
  }
}
</style>
