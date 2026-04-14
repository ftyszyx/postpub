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
  EyeOutlined,
  ReloadOutlined,
  SearchOutlined,
} from "@ant-design/icons-vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import { usePublishStore } from "../stores/publish";
import type {
  PublishEvent,
  PublishTaskStatus,
  PublishTaskSummary,
} from "../types/postpub";

type EventDisplayState = "pending" | "current" | "completed" | "failed";
type DisplayEvent = PublishEvent & {
  displayState: EventDisplayState;
};

const publishStore = usePublishStore();
const route = useRoute();
const { t, te } = useI18n();

const searchKeyword = ref("");
const detailOpen = ref(false);
const selectedTaskId = ref("");
const retryingTaskId = ref("");

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

function asTask(record: unknown) {
  return record as PublishTaskSummary;
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
    browser: "config.sections.browserEnvironmentEyebrow",
    settings: "config.sections.wechatSettingsSummary",
    prepare: "publish.stages.prepare",
    target: "publish.stages.target",
    save: "common.save",
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
    width: 180,
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
  const nextTask = await publishStore.retryTask(task);
  retryingTaskId.value = "";
  if (!nextTask) return;
  selectedTaskId.value = nextTask.id;
  detailOpen.value = true;
  message.success(t("publish.retryTaskSuccess"));
}

function taskRowProps(task: PublishTaskSummary) {
  return {
    onClick: () => openTaskDetail(task),
    style: "cursor: pointer;",
  };
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

onBeforeUnmount(() => {
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
      </template>

      <ATable
        :columns="tableColumns"
        :data-source="filteredTasks"
        :pagination="{ pageSize: 8, showSizeChanger: false }"
        :scroll="{ x: 1080 }"
        row-key="id"
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

  .task-event-list__header {
    flex-direction: column;
  }
}
</style>
