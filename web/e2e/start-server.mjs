import { spawn } from "node:child_process";

const port = process.env.ANY2API_E2E_PORT ?? "33210";
const dataDirectory = process.env.ANY2API_E2E_DATA_DIR;
if (!dataDirectory) {
  throw new Error("ANY2API_E2E_DATA_DIR is required");
}
const binary = process.env.ANY2API_E2E_BINARY;
if (!binary) {
  throw new Error("ANY2API_E2E_BINARY is required");
}
const inheritedEnvironment = Object.fromEntries(
  Object.entries(process.env).filter(
    ([name]) => !name.toLowerCase().startsWith("any2api_"),
  ),
);
const environment = {
  ...inheritedEnvironment,
  ANY2API_BIND: `127.0.0.1:${port}`,
  ANY2API_DATA_DIR: dataDirectory,
  ANY2API_ADMIN_PASSWORD: "any2api-e2e-password",
  RUST_LOG: "warn",
};
const child = spawn(binary, [], {
  cwd: dataDirectory,
  env: environment,
  stdio: "inherit",
});

let stopping = false;
function stop(signal) {
  if (stopping) return;
  stopping = true;
  if (!child.killed) child.kill(signal);
}

process.on("SIGINT", () => stop("SIGINT"));
process.on("SIGTERM", () => stop("SIGTERM"));
child.on("exit", async (code, signal) => {
  if (!stopping && code !== 0) {
    process.exitCode = code ?? (signal ? 1 : 0);
  }
});
