import { defineConfig, devices } from "@playwright/test";
import { fileURLToPath } from "node:url";

const webDirectory = fileURLToPath(new URL(".", import.meta.url));
const port = Number(process.env.ANY2API_E2E_PORT ?? "33210");
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  timeout: 30_000,
  expect: { timeout: 10_000 },
  reporter: process.env.CI ? "github" : "list",
  outputDir: "test-results",
  use: {
    baseURL,
    trace: "retain-on-failure",
  },
  webServer: {
    command: "node e2e/start-server.mjs",
    cwd: webDirectory,
    url: `${baseURL}/api/health`,
    reuseExistingServer: false,
    timeout: 120_000,
    env: {
      ...process.env,
      ANY2API_E2E_PORT: String(port),
    },
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});
