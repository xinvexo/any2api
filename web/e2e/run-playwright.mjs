import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:net";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";

const port = process.env.ANY2API_E2E_PORT ?? String(await findAvailablePort());
const dataDirectory = await mkdtemp(join(tmpdir(), "any2api-e2e-"));
const require = createRequire(import.meta.url);
const cli = require.resolve("@playwright/test/cli");
const child = spawn(process.execPath, [cli, "test"], {
  env: {
    ...process.env,
    ANY2API_E2E_PORT: port,
    ANY2API_E2E_DATA_DIR: dataDirectory,
  },
  stdio: "inherit",
});

process.once("SIGINT", () => child.kill("SIGINT"));
process.once("SIGTERM", () => child.kill("SIGTERM"));

try {
  const { code, signal } = await childExit(child);
  process.exitCode = code ?? (signal ? 1 : 0);
} finally {
  await rm(dataDirectory, { recursive: true, force: true });
}

function findAvailablePort() {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        server.close();
        reject(new Error("failed to allocate an E2E port"));
        return;
      }
      server.close((error) => (error ? reject(error) : resolve(address.port)));
    });
  });
}

function childExit(childProcess) {
  return new Promise((resolve, reject) => {
    childProcess.once("error", reject);
    childProcess.once("exit", (code, signal) => resolve({ code, signal }));
  });
}
