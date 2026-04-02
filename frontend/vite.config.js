import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

const backendTarget = process.env.POSTPUB_DEV_PROXY_TARGET || "http://127.0.0.1:3000";

export default defineConfig({
  plugins: [vue()],
  server: {
    host: "127.0.0.1",
    port: 5173,
    proxy: {
      "/api": {
        target: backendTarget,
        changeOrigin: true
      },
      "/images": {
        target: backendTarget,
        changeOrigin: true
      }
    }
  }
});
