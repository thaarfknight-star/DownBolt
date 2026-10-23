import { defineConfig } from "vite";

// فرانت داخل src/ است؛ خروجی بیلد باید به dist/ کنار src-tauri برود
// (همان frontendDist در tauri.conf.json).
export default defineConfig({
  root: "src",
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    strictPort: true,
  },
  clearScreen: false,
});
