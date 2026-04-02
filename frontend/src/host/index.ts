import { browserHostBridge } from "./browser";
import type { HostBridge } from "./types";

function detectHostBridge(): HostBridge {
  if (window.__TAURI__) {
    return {
      async openExternal(url: string) {
        console.info("Desktop host bridge placeholder:", url);
      },
      getEnvironmentLabel() {
        return "desktop-placeholder";
      }
    };
  }

  return browserHostBridge;
}

export const hostBridge = detectHostBridge();
export type { HostBridge } from "./types";
