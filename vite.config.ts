/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, host: "127.0.0.1" },
  envPrefix: ["VITE_", "TAURI_"],
  test: {
    // output/ 是冻结的发布快照（非活源码），不要被 vitest 扫入
    exclude: ["node_modules/**", "dist/**", "output/**", "src-tauri/target/**"],
  },
});
