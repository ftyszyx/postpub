import { defineStore } from "pinia";
import {
  apiDelete,
  apiGet,
  apiPost,
  apiPut,
  encodePathSegments,
  type ApiResponse
} from "../api/client";
import type {
  TemplateCategorySummary,
  TemplateDocument,
  TemplateSummary
} from "../types/postpub";
import { translate } from "../utils/i18n";

interface LoadAllOptions {
  preserveMessage?: boolean;
}

interface TemplateActionOptions {
  focusResult?: boolean;
  reloadCategory?: string;
}

export const useTemplateStore = defineStore("templates", {
  state: () => ({
    categories: [] as TemplateCategorySummary[],
    templates: [] as TemplateSummary[],
    current: null as TemplateDocument | null,
    selectedCategory: "",
    loading: false,
    saving: false,
    error: "",
    lastMessage: ""
  }),

  actions: {
    async loadAll(category?: string, options: LoadAllOptions = {}) {
      this.loading = true;
      this.error = "";
      if (!options.preserveMessage) {
        this.lastMessage = "";
      }

      try {
        const [categories, templates] = await Promise.all([
          apiGet<ApiResponse<TemplateCategorySummary[]>>("/api/templates/categories"),
          apiGet<ApiResponse<TemplateSummary[]>>("/api/templates", category ? { category } : undefined)
        ]);
        this.categories = categories.data;
        this.templates = templates.data;
        this.selectedCategory = category || "";
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
        const response = await apiGet<ApiResponse<TemplateDocument>>(
          `/api/templates/${encodePathSegments(relativePath)}`
        );
        this.current = response.data;
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
    },

    async createCategory(name: string) {
      this.error = "";
      this.lastMessage = "";

      try {
        await apiPost<ApiResponse<TemplateCategorySummary>>("/api/templates/categories", { name });
        await this.loadAll(name, { preserveMessage: true });
        this.lastMessage = translate("messages.templates.categoryCreated");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    },

    async renameCategory(currentName: string, newName: string) {
      this.error = "";
      this.lastMessage = "";

      try {
        await apiPut<ApiResponse<{ category_name: string; new_name: string }>>(
          `/api/templates/categories/${encodeURIComponent(currentName)}`,
          { new_name: newName }
        );
        if (this.current?.category === currentName) {
          this.current.category = newName;
          this.current.relative_path = this.current.relative_path.replace(
            `${currentName}/`,
            `${newName}/`
          );
        }

        await this.loadAll(newName, { preserveMessage: true });
        this.lastMessage = translate("messages.templates.categoryRenamed");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    },

    async deleteCategory(name: string) {
      this.error = "";
      this.lastMessage = "";

      try {
        await apiDelete<ApiResponse<{ category_name: string }>>(
          `/api/templates/categories/${encodeURIComponent(name)}`
        );
        if (this.current?.category === name) {
          this.current = null;
        }
        await this.loadAll(undefined, { preserveMessage: true });
        this.lastMessage = translate("messages.templates.categoryDeleted");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    },

    async createTemplate(name: string, category: string, content: string) {
      this.error = "";
      this.lastMessage = "";

      try {
        const response = await apiPost<ApiResponse<TemplateDocument>>("/api/templates", {
          name,
          category,
          content
        });
        this.current = response.data;
        await this.loadAll(category, { preserveMessage: true });
        this.lastMessage = translate("messages.templates.templateCreated");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    },

    async saveCurrent() {
      if (!this.current) {
        return;
      }

      this.saving = true;
      this.error = "";
      try {
        const response = await apiPut<ApiResponse<TemplateDocument>>(
          `/api/templates/${encodePathSegments(this.current.relative_path)}`,
          { content: this.current.content }
        );
        this.current = response.data;
        await this.loadAll(this.current.category, { preserveMessage: true });
        this.lastMessage = translate("messages.templates.templateSaved");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.saving = false;
      }
    },

    async deleteTemplate(relativePath: string, reloadCategory?: string) {
      this.error = "";
      this.lastMessage = "";
      const nextCategory = reloadCategory ?? (this.selectedCategory || undefined);

      try {
        await apiDelete<ApiResponse<{ relative_path: string }>>(
          `/api/templates/${encodePathSegments(relativePath)}`
        );
        if (this.current?.relative_path === relativePath) {
          this.current = null;
        }
        await this.loadAll(nextCategory, { preserveMessage: true });
        this.lastMessage = translate("messages.templates.templateDeleted");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    },

    async renameTemplate(relativePath: string, newName: string, options: TemplateActionOptions = {}) {
      this.error = "";
      this.lastMessage = "";

      try {
        const response = await apiPost<ApiResponse<TemplateDocument>>("/api/templates/actions/rename", {
          relative_path: relativePath,
          new_name: newName
        });

        if (options.focusResult || this.current?.relative_path === relativePath) {
          this.current = response.data;
        }

        await this.loadAll(options.reloadCategory, { preserveMessage: true });
        this.lastMessage = translate("messages.templates.templateRenamed");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    },

    async copyTemplate(
      relativePath: string,
      targetCategory: string,
      newName: string,
      options: TemplateActionOptions = {}
    ) {
      this.error = "";
      this.lastMessage = "";

      try {
        const response = await apiPost<ApiResponse<TemplateDocument>>("/api/templates/actions/copy", {
          relative_path: relativePath,
          target_category: targetCategory,
          new_name: newName
        });

        if (options.focusResult) {
          this.current = response.data;
        }

        await this.loadAll(options.reloadCategory, { preserveMessage: true });
        this.lastMessage = translate("messages.templates.templateCopied");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    },

    async moveTemplate(
      relativePath: string,
      targetCategory: string,
      options: TemplateActionOptions = {}
    ) {
      this.error = "";
      this.lastMessage = "";

      try {
        const response = await apiPost<ApiResponse<TemplateDocument>>("/api/templates/actions/move", {
          relative_path: relativePath,
          target_category: targetCategory
        });

        if (options.focusResult || this.current?.relative_path === relativePath) {
          this.current = response.data;
        }

        await this.loadAll(options.reloadCategory, { preserveMessage: true });
        this.lastMessage = translate("messages.templates.templateMoved");
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      }
    },

    async deleteCurrent() {
      if (!this.current) {
        return;
      }

      await this.deleteTemplate(this.current.relative_path, this.current.category);
    },

    async renameCurrent(newName: string) {
      if (!this.current) {
        return;
      }

      await this.renameTemplate(this.current.relative_path, newName, {
        focusResult: true,
        reloadCategory: this.current.category
      });
    },

    async copyCurrent(targetCategory: string, newName: string) {
      if (!this.current) {
        return;
      }

      await this.copyTemplate(this.current.relative_path, targetCategory, newName, {
        focusResult: true,
        reloadCategory: targetCategory
      });
    },

    async moveCurrent(targetCategory: string) {
      if (!this.current) {
        return;
      }

      await this.moveTemplate(this.current.relative_path, targetCategory, {
        focusResult: true,
        reloadCategory: targetCategory
      });
    }
  }
});
