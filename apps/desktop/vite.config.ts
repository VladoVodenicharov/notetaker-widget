import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed port and its own error output, so keep Vite quiet and strict.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  // Loaded from disk by the desktop app, so bundle size barely matters.
  build: { chunkSizeWarningLimit: 1500 },
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
