import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { fileURLToPath } from "node:url";
import adapter from "@sveltejs/adapter-static";

const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  // Tauri has no Node server, so prerender the app as a static SPA (SSG).
  plugins: [
    sveltekit({
      adapter: adapter({ fallback: "index.html" }),
      prerender: { handleUnseenRoutes: "ignore" },
    }),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  resolve: {
    alias: {
      "@mediapipe/pose": fileURLToPath(
        new URL("./src/lib/shims/mediapipe-pose.js", import.meta.url),
      ),
    },
  },
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
