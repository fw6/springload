import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

// jsPDF 的 `html()` 与 `svg()` 动态 import 的三个可选依赖指到这里（见该文件说明）。
const NOT_BUNDLED = fileURLToPath(new URL("./src/lib/not-bundled.ts", import.meta.url));

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],

  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
      html2canvas: NOT_BUNDLED,
      canvg: NOT_BUNDLED,
      dompurify: NOT_BUNDLED,
    },
  },

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },

  // 测试（vitest + jsdom；测试里显式 import，不开 globals）。
  test: {
    environment: "jsdom",
    setupFiles: ["src/test-setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
