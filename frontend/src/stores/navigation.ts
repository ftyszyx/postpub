import { defineStore } from "pinia";

export type ConfigPanel = "ui" | "image-models" | "llm-models" | "publish-platforms";

function normalizeConfigPanel(panel: string | null | undefined): ConfigPanel {
  if (
    panel === "ui" ||
    panel === "image-models" ||
    panel === "llm-models" ||
    panel === "publish-platforms"
  ) {
    return panel;
  }

  if (panel === "image-design") {
    return "ui";
  }

  if (panel === "api") {
    return "llm-models";
  }

  if (panel === "base" || panel === "platforms" || panel === "aiforge" || panel === "creative") {
    return "publish-platforms";
  }

  return "ui";
}

export const useNavigationStore = defineStore("navigation", {
  state: () => ({
    activeConfigPanel: "ui" as ConfigPanel
  }),

  actions: {
    setActiveConfigPanel(panel: string | null | undefined) {
      this.activeConfigPanel = normalizeConfigPanel(panel);
    }
  }
});
