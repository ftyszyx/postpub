import { defineStore } from "pinia";
import { apiDelete, apiGet, apiPost, type ApiResponse } from "../api/client";
import type {
  PublishEvent,
  PublishTaskStatus,
  PublishTaskSummary
} from "../types/postpub";

function normalizeTask(task: PublishTaskSummary): PublishTaskSummary {
  return {
    ...task,
    events: task.events ?? []
  };
}

function normalizeTaskIds(taskIds: string[]): string[] {
  return [...new Set(taskIds.map((taskId) => taskId.trim()).filter(Boolean))];
}

export const usePublishStore = defineStore("publish", {
  state: () => ({
    tasks: [] as PublishTaskSummary[],
    current: null as PublishTaskSummary | null,
    loading: false,
    error: "",
    eventSource: null as EventSource | null
  }),

  actions: {
    async loadTasks() {
      this.loading = true;
      this.error = "";

      try {
        const response = await apiGet<ApiResponse<PublishTaskSummary[]>>("/api/publish/tasks");
        const currentId = this.current?.id;
        this.tasks = response.data.map(normalizeTask);
        this.current =
          this.tasks.find((task) => task.id === currentId) ||
          this.current ||
          this.tasks[0] ||
          null;

        if (
          this.current &&
          (this.current.status === "Pending" || this.current.status === "Running")
        ) {
          this.connectToEvents(this.current.id);
        }
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
    },

    async retryTask(task: PublishTaskSummary) {
      this.error = "";

      try {
        const response = await apiPost<ApiResponse<PublishTaskSummary>>(
          `/api/publish/tasks/${encodeURIComponent(task.id)}/retry`
        );
        const nextTask = normalizeTask(response.data);
        this.current = nextTask;
        this.tasks = this.tasks.map((item) => (item.id === nextTask.id ? nextTask : item));
        await this.loadTasks();
        this.connectToEvents(nextTask.id);
        return this.current;
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
        return null;
      }
    },

    async deleteTask(taskId: string) {
      this.error = "";

      try {
        await apiDelete<ApiResponse<{ task_id: string }>>(
          `/api/publish/tasks/${encodeURIComponent(taskId)}`
        );
        this.removeTasks([taskId]);
        return true;
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
        return false;
      }
    },

    async deleteTasks(taskIds: string[]) {
      const normalizedIds = normalizeTaskIds(taskIds);
      if (!normalizedIds.length) {
        return [];
      }

      this.error = "";

      try {
        const response = await apiPost<ApiResponse<{ task_ids: string[]; deleted_count: number }>>(
          "/api/publish/tasks/actions/delete",
          {
            ids: normalizedIds
          }
        );
        const deletedIds = normalizeTaskIds(response.data.task_ids ?? normalizedIds);
        this.removeTasks(deletedIds);
        return deletedIds;
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
        return [];
      }
    },

    async selectTask(taskId: string) {
      try {
        const response = await apiGet<ApiResponse<PublishTaskSummary>>(
          `/api/publish/tasks/${encodeURIComponent(taskId)}`
        );
        this.current = normalizeTask(response.data);
        if (this.current.status === "Pending" || this.current.status === "Running") {
          this.connectToEvents(taskId);
        }
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    },

    connectToEvents(taskId: string) {
      this.eventSource?.close();
      const source = new EventSource(`/api/publish/tasks/${encodeURIComponent(taskId)}/events`);
      source.onmessage = (event) => {
        try {
          const rawPayload = JSON.parse(event.data) as {
            task_id: string;
            stage: string;
            message: string;
            status: string;
            timestamp: string;
          };
          const payload: PublishEvent = {
            ...rawPayload,
            status: rawPayload.status as PublishTaskStatus
          };

          this.tasks = this.tasks.map((task) =>
            task.id === taskId
              ? {
                  ...task,
                  events: [...task.events, payload],
                  status: payload.status as PublishTaskSummary["status"],
                  updated_at: payload.timestamp
                }
              : task
          );

          if (this.current?.id === taskId) {
            this.current.events = [...this.current.events, payload];
            this.current.status = payload.status as PublishTaskSummary["status"];
            this.current.updated_at = payload.timestamp;
          }

          if (payload.status === "Succeeded" || payload.status === "Failed") {
            source.close();
            void this.selectTask(taskId);
            void this.loadTasks();
          }
        } catch (error) {
          console.error(error);
        }
      };
      source.onerror = () => {
        source.close();
      };
      this.eventSource = source;
    },

    removeTasks(taskIds: string[]) {
      const deletedIds = new Set(normalizeTaskIds(taskIds));
      if (!deletedIds.size) {
        return;
      }

      const currentId = this.current?.id ?? null;
      if (currentId && deletedIds.has(currentId)) {
        this.eventSource?.close();
        this.eventSource = null;
      }

      this.tasks = this.tasks.filter((task) => !deletedIds.has(task.id));

      if (!currentId) {
        this.current = this.tasks[0] ?? null;
        return;
      }

      if (deletedIds.has(currentId)) {
        this.current = this.tasks[0] ?? null;
        return;
      }

      this.current = this.tasks.find((task) => task.id === currentId) ?? this.current;
    },

    dispose() {
      this.eventSource?.close();
      this.eventSource = null;
    }
  }
});
