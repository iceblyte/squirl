/// <reference types="vitest" />
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Tauri 前端构建（设计 §4.2）：Vite 8，端口固定 5173 与 tauri.conf.json 对齐
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: "es2022",
  },
  test: {
    environment: "jsdom",
    globals: true, // RTL 自动 cleanup 依赖全局 afterEach
  },
});
