import { spawn } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:net";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repository = fileURLToPath(new URL("../..", import.meta.url));
const chromiumUnsafePorts = new Set([
  1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95,
  101, 102, 103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161,
  179, 389, 427, 465, 512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563,
  587, 601, 636, 989, 990, 993, 995, 1719, 1720, 1723, 2049, 3659, 4045, 5060, 5061,
  6000, 6566, 6665, 6666, 6667, 6668, 6669, 6697, 10080,
]);
let activeChild;
let interruptedSignal;
process.once("SIGINT", () => interrupt("SIGINT"));
process.once("SIGTERM", () => interrupt("SIGTERM"));

const binary = resolve(
  repository,
  process.env.ANY2API_E2E_BINARY ?? (await buildServer(repository)),
);
const port = process.env.ANY2API_E2E_PORT ?? String(await findAvailablePort());
const dataDirectory = await mkdtemp(join(tmpdir(), "any2api-e2e-"));
const require = createRequire(import.meta.url);
const cli = require.resolve("@playwright/test/cli");

try {
  if (interruptedSignal) {
    throw new Error(`E2E interrupted by ${interruptedSignal}`);
  }
  const child = spawn(process.execPath, [cli, "test"], {
    env: {
      ...process.env,
      ANY2API_E2E_PORT: port,
      ANY2API_E2E_DATA_DIR: dataDirectory,
      ANY2API_E2E_BINARY: binary,
    },
    stdio: "inherit",
  });
  activeChild = child;
  const { code, signal } = await childClose(child);
  process.exitCode = code ?? (signal ? 1 : 0);
} finally {
  activeChild = undefined;
  await rm(dataDirectory, { recursive: true, force: true });
}

async function buildServer(repositoryDirectory) {
  const cargo = spawn(
    "cargo",
    [
      "build",
      "--locked",
      "--manifest-path",
      join(repositoryDirectory, "Cargo.toml"),
      "-p",
      "any2api",
      "--message-format=json-render-diagnostics",
    ],
    {
      cwd: repositoryDirectory,
      stdio: ["ignore", "pipe", "inherit"],
    },
  );
  activeChild = cargo;
  cargo.stdout.setEncoding("utf8");
  let output = "";
  cargo.stdout.on("data", (chunk) => {
    output += chunk;
  });

  const { code, signal } = await childClose(cargo);
  activeChild = undefined;
  const executable = cargoExecutable(output);
  if (interruptedSignal) {
    throw new Error(`cargo build interrupted by ${interruptedSignal}`);
  }
  if (code !== 0) {
    throw new Error(`cargo build failed with ${code ?? signal ?? "unknown status"}`);
  }
  if (!executable) {
    throw new Error("cargo build did not report the any2api executable");
  }
  return executable;
}

function interrupt(signal) {
  interruptedSignal = signal;
  if (activeChild && !activeChild.killed) activeChild.kill(signal);
}

function cargoExecutable(output) {
  let executable;
  for (const line of output.split(/\r?\n/u)) {
    if (!line) continue;
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      process.stdout.write(`${line}\n`);
      continue;
    }
    if (message.reason === "compiler-message" && message.message?.rendered) {
      process.stderr.write(message.message.rendered);
    }
    if (
      message.reason === "compiler-artifact" &&
      message.target?.name === "any2api" &&
      message.target?.kind?.includes("bin") &&
      message.executable
    ) {
      executable = message.executable;
    }
  }
  return executable;
}

async function findAvailablePort() {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    const port = await allocateAvailablePort();
    if (!chromiumUnsafePorts.has(port)) return port;
  }
  throw new Error("failed to allocate a Chromium-safe E2E port");
}

function allocateAvailablePort() {
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

function childClose(childProcess) {
  return new Promise((resolve, reject) => {
    childProcess.once("error", reject);
    childProcess.once("close", (code, signal) => resolve({ code, signal }));
  });
}
