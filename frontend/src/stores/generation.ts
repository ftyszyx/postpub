import { defineStore } from "pinia";
import { apiGet, apiPost, type ApiResponse } from "../api/client";
import type {
  GenerateArticleRequest,
  GenerationEvent,
  GenerationTaskStatus,
  GenerationTaskSummary
} from "../types/postpub";

function emptyRequest(): GenerateArticleRequest {
  return {
    topic: "",
    platform: "web",
    reference_urls: [],
    reference_ratio: 0.3,
    template_category: "general",
    template_name: "magazine",
    save_output: true
  };
}

export const useGenerationStore = defineStore("generation", {
  state: () => ({
    form: emptyRequest(),
    tasks: [] as GenerationTaskSummary[],
    current: null as GenerationTaskSummary | null,
    creating: false,
    loading: false,
    error: "",
    eventSource: null as EventSource | null
  }),

  actions: {
    async loadTasks() {
      this.loading = true;
      this.error = "";

      try {
        const response = await apiGet<ApiResponse<GenerationTaskSummary[]>>("/api/generation/tasks");
        const currentId = this.current?.id;
        this.tasks = response.data;
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

    async createTask() {
      this.creating = true;
      this.error = "";

      try {
        const response = await apiPost<ApiResponse<GenerationTaskSummary>>(
          "/api/generation/tasks",
          this.form
        );
        this.current = response.data;
        await this.loadTasks();
        this.connectToEvents(response.data.id);
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.creating = false;
      }
    },

    async selectTask(taskId: string) {
      try {
        const response = await apiGet<ApiResponse<GenerationTaskSummary>>(
          `/api/generation/tasks/${encodeURIComponent(taskId)}`
        );
        this.current = response.data;
        if (this.current.status === "Pending" || this.current.status === "Running") {
          this.connectToEvents(taskId);
        }
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    },

    connectToEvents(taskId: string) {
      this.eventSource?.close();
      const source = new EventSource(`/api/generation/tasks/${encodeURIComponent(taskId)}/events`);
      source.onmessage = (event) => {
        try {
          const rawPayload = JSON.parse(event.data) as {
            task_id: string;
            stage: string;
            message: string;
            status: string;
            timestamp: string;
          };
          const payload: GenerationEvent = {
            ...rawPayload,
            status: rawPayload.status as GenerationTaskStatus
          };

          if (!this.current || this.current.id !== taskId) {
            return;
          }

          this.current.events = [...this.current.events, payload];
          this.current.status = payload.status as GenerationTaskSummary["status"];

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

    dispose() {
      this.eventSource?.close();
      this.eventSource = null;
    }
  }
});
