<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useI18n } from "vue-i18n";
import GenerationTasksView from "./GenerationTasksView.vue";
import PublishTasksView from "./PublishTasksView.vue";
import { useGenerationStore } from "../stores/generation";
import { usePublishStore } from "../stores/publish";

type TaskKind = "generation" | "publish";

const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const generationStore = useGenerationStore();
const publishStore = usePublishStore();

const activeTaskKind = ref<TaskKind>("generation");

const generationTaskCount = computed(() => generationStore.tasks.length);
const publishTaskCount = computed(() => publishStore.tasks.length);

function parseTaskKind(value: unknown): TaskKind | null {
  return value === "generation" || value === "publish" ? value : null;
}

function hasActiveTask(kind: TaskKind) {
  const tasks = kind === "publish" ? publishStore.tasks : generationStore.tasks;
  return tasks.some((task) => task.status === "Pending" || task.status === "Running");
}

function latestUpdatedAt(kind: TaskKind) {
  const tasks = kind === "publish" ? publishStore.tasks : generationStore.tasks;
  return tasks
    .map((task) => Date.parse(task.updated_at))
    .filter((value) => !Number.isNaN(value))
    .sort((left, right) => right - left)[0] || 0;
}

function preferredTaskKindFromData(): TaskKind {
  if (hasActiveTask("publish")) return "publish";
  if (hasActiveTask("generation")) return "generation";
  return latestUpdatedAt("publish") > latestUpdatedAt("generation") ? "publish" : "generation";
}

watch(
  () => route.query.kind,
  (kind) => {
    const next = parseTaskKind(kind);
    if (next) {
      activeTaskKind.value = next;
    }
  },
  { immediate: true }
);

watch(activeTaskKind, (kind) => {
  const currentKind = parseTaskKind(route.query.kind);
  if (currentKind === kind) {
    return;
  }

  void router.replace({
    query: {
      ...route.query,
      kind
    }
  });
});

onMounted(async () => {
  await Promise.all([generationStore.loadTasks(), publishStore.loadTasks()]);

  const kindFromRoute = parseTaskKind(route.query.kind);
  activeTaskKind.value = kindFromRoute ?? preferredTaskKindFromData();
});
</script>

<template>
  <section class="task-status-shell">
    <div class="task-kind-switch" role="tablist" :aria-label="t('taskStatusPage.switchLabel')">
      <button
        class="task-kind-switch__item"
        :class="{ 'task-kind-switch__item--active': activeTaskKind === 'generation' }"
        type="button"
        role="tab"
        :aria-selected="activeTaskKind === 'generation'"
        @click="activeTaskKind = 'generation'"
      >
        <span>{{ t("taskStatusPage.tabs.generation") }}</span>
        <strong>{{ generationTaskCount }}</strong>
      </button>
      <button
        class="task-kind-switch__item"
        :class="{ 'task-kind-switch__item--active': activeTaskKind === 'publish' }"
        type="button"
        role="tab"
        :aria-selected="activeTaskKind === 'publish'"
        @click="activeTaskKind = 'publish'"
      >
        <span>{{ t("taskStatusPage.tabs.publish") }}</span>
        <strong>{{ publishTaskCount }}</strong>
      </button>
    </div>

    <GenerationTasksView v-if="activeTaskKind === 'generation'" />
    <PublishTasksView v-else />
  </section>
</template>

<style scoped>
.task-status-shell {
  display: grid;
  gap: 18px;
}

.task-kind-switch {
  display: inline-flex;
  gap: 8px;
  padding: 6px;
  border-radius: 18px;
  background: color-mix(in srgb, var(--surface-raised) 92%, white);
  border: 1px solid var(--border-soft);
  width: fit-content;
  max-width: 100%;
}

.task-kind-switch__item {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  border: 0;
  border-radius: 12px;
  padding: 10px 16px;
  background: transparent;
  color: var(--ink-muted);
  cursor: pointer;
  transition: background-color 0.18s ease, color 0.18s ease, box-shadow 0.18s ease;
}

.task-kind-switch__item strong {
  min-width: 1.5rem;
  text-align: center;
}

.task-kind-switch__item--active {
  background: var(--primary);
  color: white;
  box-shadow: 0 12px 24px color-mix(in srgb, var(--primary) 25%, transparent);
}

@media (max-width: 720px) {
  .task-kind-switch {
    width: 100%;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .task-kind-switch__item {
    justify-content: center;
  }
}
</style>
