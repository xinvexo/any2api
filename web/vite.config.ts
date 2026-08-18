import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "VITE_");
  const applicationOutput = process.env.ANY2API_WEB_OUTPUT_DIR;

  return {
    plugins: [react(), tailwindcss()],
    resolve: {
      alias: {
        "@": fileURLToPath(new URL("./src", import.meta.url)),
      },
    },
    server: {
      host: "127.0.0.1",
      port: 5173,
      strictPort: true,
      // Keep terminal output readable when the root development supervisor runs Vite.
      clearScreen: false,
      proxy: {
        "/api": {
          target:
            process.env.VITE_API_TARGET
            || env.VITE_API_TARGET
            || "http://127.0.0.1:3210",
          changeOrigin: true,
        },
      },
    },
    build: applicationOutput
      ? {
          outDir: applicationOutput,
          emptyOutDir: true,
        }
      : undefined,
    test: {
      include: ["src/**/*.test.{ts,tsx}"],
      environment: "jsdom",
      setupFiles: "./src/test/setup.ts",
      css: true,
    },
  };
});
