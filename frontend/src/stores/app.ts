import { defineStore } from "pinia";
import { apiGet, type ApiResponse } from "../api/client";
import type { AppPathsInfo, HealthStatus } from "../types/postpub";

interface AppState {
  health: HealthStatus | null;
  paths: AppPathsInfo | null;
  loading: boolean;
  error: string;
}

export const useAppStore = defineStore("app", {
  state: (): AppState => ({
    health: null,
    paths: null,
    loading: false,
    error: ""
  }),

  actions: {
    async refreshSystem() {
      this.loading = true;
      this.error = "";

      try {
        const [health, paths] = await Promise.all([
          apiGet<ApiResponse<HealthStatus>>("/api/system/health"),
          apiGet<ApiResponse<AppPathsInfo>>("/api/system/paths")
        ]);
        this.health = health.data;
        this.paths = paths.data;
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
    },

    clearError() {
      this.error = "";
    }
  }
});
