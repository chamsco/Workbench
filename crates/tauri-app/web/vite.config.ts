import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

// One ES module + one stylesheet, at fixed names the Tauri UI loads.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: [
      { find: "@", replacement: fileURLToPath(new URL("./src/whirl", import.meta.url)) },
      // Every grammar shiki ships is ~10 MB; see src/shiki-slim.ts.
      { find: /^shiki$/, replacement: fileURLToPath(new URL("./src/shiki-slim.ts", import.meta.url)) },
    ],
  },
  base: "./",
  build: {
    outDir: "../ui/chat-web",
    emptyOutDir: true,
    cssCodeSplit: false,
    chunkSizeWarningLimit: 4000,
    rollupOptions: {
      input: "src/main.tsx",
      output: {
        format: "es",
        entryFileNames: "whirl.js",
        chunkFileNames: "whirl-[name]-[hash].js",
        assetFileNames: (a) => (a.names?.[0]?.endsWith(".css") ? "whirl.css" : "assets/[name][extname]"),
      },
      preserveEntrySignatures: "exports-only",
    },
  },
});
