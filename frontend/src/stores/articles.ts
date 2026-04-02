import { defineStore } from "pinia";
import {
  apiDelete,
  apiGet,
  apiPut,
  encodePathSegments,
  type ApiResponse
} from "../api/client";
import type { ArticleDocument, ArticleSummary } from "../types/postpub";
import { translate } from "../utils/i18n";

export const useArticleStore = defineStore("articles", {
  state: () => ({
    articles: [] as ArticleSummary[],
    current: null as ArticleDocument | null,
    loading: false,
    saving: false,
    error: "",
    lastMessage: ""
  }),

  actions: {
    async loadAll() {
      this.loading = true;
      this.error = "";
      this.lastMessage = "";

      try {
        const response = await apiGet<ApiResponse<ArticleSummary[]>>("/api/articles");
        this.articles = response.data;
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
    },

    async open(relativePath: string) {
      this.loading = true;
      this.error = "";

      try {
        const response = await apiGet<ApiResponse<ArticleDocument>>(
          `/api/articles/${encodePathSegments(relativePath)}`
        );
        this.current = response.data;
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
    },

    async saveCurrent() {
      if (!this.current) {
        return;
      }

      this.saving = true;
      this.error = "";
      this.lastMessage = "";
      try {
        const response = await apiPut<ApiResponse<ArticleDocument>>(
          `/api/articles/${encodePathSegments(this.current.summary.relative_path)}`,
          { content: this.current.content }
        );
        this.current = response.data;
        this.lastMessage = translate("messages.articles.saved");
        await this.loadAll();
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.saving = false;
      }
    },

    async deleteCurrent() {
      if (!this.current) {
        return;
      }

      const relativePath = this.current.summary.relative_path;
      this.error = "";
      this.lastMessage = "";

      try {
        await apiDelete<ApiResponse<{ relative_path: string }>>(
          `/api/articles/${encodePathSegments(relativePath)}`
        );
        this.current = null;
        this.lastMessage = translate("messages.articles.deleted");
        await this.loadAll();
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    }
  }
});
