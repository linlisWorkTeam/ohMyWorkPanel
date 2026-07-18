import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Web build config — stubs out Tauri modules for pure browser deployment
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    outDir: "dist",
  },
  resolve: {
    alias: [
      {
        find: "@tauri-apps/api/core",
        replacement: path.resolve(__dirname, "src/stubs/tauri-core.ts"),
      },
      {
        find: "@tauri-apps/plugin-dialog",
        replacement: path.resolve(__dirname, "src/stubs/tauri-dialog.ts"),
      },
      {
        find: "@tauri-apps/api/event",
        replacement: path.resolve(__dirname, "src/stubs/tauri-event.ts"),
      },
      // Redirect ./api to the web API layer (instead of Tauri invoke)
      {
        find: /^\.\/api$/,
        replacement: path.resolve(__dirname, "src/api-web.ts"),
      },
    ],
  },
});
