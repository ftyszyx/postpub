import { defineStore } from "pinia";
import { apiGet, apiPost, type ApiResponse } from "../api/client";
import type {
  ArticleSummary,
  GenerateArticleRequest,
  GenerationEvent,
  GenerationTaskStatus,
  GenerationOutput,
  GenerationTaskSummary
} from "../types/postpub";

function emptyRequest(): GenerateArticleRequest {
  return {
    topic: "",
    reference_urls: [],
    template_category: "general",
    template_name: "magazine",
    save_output: true
  };
}

function cloneRequest(request: GenerateArticleRequest): GenerateArticleRequest {
  return {
    ...request,
    reference_urls: [...request.reference_urls]
  };
}

function normalizeArticleSummary(article?: ArticleSummary | null): ArticleSummary | undefined {
  if (!article) {
    return undefined;
  }

  return {
    ...article,
    variant_count: article.variant_count ?? 0
  };
}

function normalizeOutput(output?: GenerationOutput | null): GenerationOutput | undefined {
  if (!output) {
    return undefined;
  }

  return {
    ...output,
    variants: output.variants ?? [],
    article: normalizeArticleSummary(output.article)
  };
}

function normalizeTask(task: GenerationTaskSummary): GenerationTaskSummary {
  return {
    ...task,
    output: normalizeOutput(task.output)
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

    async createTask(request?: GenerateArticleRequest) {
      this.creating = true;
      this.error = "";

      try {
        const payload = cloneRequest(request ?? this.form);
        const response = await apiPost<ApiResponse<GenerationTaskSummary>>(
          "/api/generation/tasks",
          payload
        );
        this.current = normalizeTask(response.data);
        await this.loadTasks();
        this.connectToEvents(this.current.id);
        return this.current;
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
        return null;
      } finally {
        this.creating = false;
      }
    },

    async retryTask(task: GenerationTaskSummary) {
      this.error = "";

      try {
        const response = await apiPost<ApiResponse<GenerationTaskSummary>>(
          `/api/generation/tasks/${encodeURIComponent(task.id)}/retry`
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

    async selectTask(taskId: string) {
      try {
        const response = await apiGet<ApiResponse<GenerationTaskSummary>>(
          `/api/generation/tasks/${encodeURIComponent(taskId)}`
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
            this.tasks = this.tasks.map((task) =>
              task.id === taskId
                ? {
                    ...task,
                    events: [...task.events, payload],
                    status: payload.status as GenerationTaskSummary["status"],
                    updated_at: payload.timestamp
                  }
                : task
            );
            return;
          }

          this.tasks = this.tasks.map((task) =>
            task.id === taskId
              ? {
                  ...task,
                  events: [...task.events, payload],
                  status: payload.status as GenerationTaskSummary["status"],
                  updated_at: payload.timestamp
                }
              : task
          );
          this.current.events = [...this.current.events, payload];
          this.current.status = payload.status as GenerationTaskSummary["status"];
          this.current.updated_at = payload.timestamp;

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
