import type { HostBridge } from "./types";

export const browserHostBridge: HostBridge = {
  async openExternal(url: string) {
    window.open(url, "_blank", "noopener,noreferrer");
  },

  getEnvironmentLabel() {
    return "browser";
  }
};
