import { spawnSync } from "node:child_process";
import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv, type Plugin } from "vite";

function syncEmbeddedWebPlugin(): Plugin {
  let buildFailed = false;

  return {
    name: "sync-embedded-web",
    apply: "build",
    buildStart() {
      buildFailed = false;
    },
    buildEnd(error) {
      if (error) {
        buildFailed = true;
      }
    },
    closeBundle() {
      if (buildFailed || process.env.VITE_SKIP_EMBEDDED_SYNC === "1") {
        return;
      }

      const script = fileURLToPath(new URL("./scripts/sync-embedded-web.mjs", import.meta.url));
      const result = spawnSync(process.execPath, [script], {
        stdio: "inherit",
        env: process.env,
      });
      if (result.status !== 0) {
        throw new Error("failed to synchronize embedded web assets");
      }
    },
  };
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "VITE_");

  return {
    plugins: [react(), tailwindcss(), syncEmbeddedWebPlugin()],
    resolve: {
      alias: {
        "@": fileURLToPath(new URL("./src", import.meta.url)),
      },
    },
    server: {
      proxy: {
        "/api": {
          target: env.VITE_API_TARGET || "http://127.0.0.1:3210",
          changeOrigin: true,
        },
      },
    },
    test: {
      include: ["src/**/*.test.{ts,tsx}"],
      environment: "jsdom",
      setupFiles: "./src/test/setup.ts",
      css: true,
    },
  };
});
