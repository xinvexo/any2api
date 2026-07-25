#!/usr/bin/env node
/**
 * Local development runner with auto-reload.
 *
 * - Backend: `cargo run --package any2api --bin any2api`, restarts on Rust/migration changes
 * - Frontend: Vite HMR (`pnpm dev:server`), open the printed URL for instant UI refresh
 *
 * Usage (repo root):
 *   node scripts/dev.mjs
 *   pnpm --dir web dev:app
 *
 * Open the Vite URL (default http://127.0.0.1:5173). `/api` is proxied to the backend.
 * Do not use the backend origin for UI work — that serves compile-time embedded assets.
 */

import { spawn, spawnSync } from "node:child_process";
import { existsSync, watch } from "node:fs";
import { relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const webDir = resolve(root, "web");
const backendArgs = ["run", "--package", "any2api", "--bin", "any2api"];
const watchRoots = ["app", "crates", "migrations", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"];
const restartExtensions = new Set([".rs", ".toml", ".sql", ".lock"]);
const debounceMs = 400;
const stopWaitMs = 10_000;

/** @type {import('node:child_process').ChildProcess | null} */
let backend = null;
/** @type {import('node:child_process').ChildProcess | null} */
let frontend = null;
/** @type {ReturnType<typeof setTimeout> | null} */
let restartTimer = null;
let shuttingDown = false;
let restartGeneration = 0;

const color = {
  dim: (text) => `\x1b[2m${text}\x1b[0m`,
  cyan: (text) => `\x1b[36m${text}\x1b[0m`,
  green: (text) => `\x1b[32m${text}\x1b[0m`,
  yellow: (text) => `\x1b[33m${text}\x1b[0m`,
  red: (text) => `\x1b[31m${text}\x1b[0m`,
  bold: (text) => `\x1b[1m${text}\x1b[0m`,
};

function log(message) {
  console.log(`${color.cyan("[dev]")} ${message}`);
}

function logWarn(message) {
  console.warn(`${color.yellow("[dev]")} ${message}`);
}

function logError(message) {
  console.error(`${color.red("[dev]")} ${message}`);
}

function ensureLayout() {
  if (!existsSync(resolve(root, "Cargo.toml"))) {
    throw new Error(`expected workspace Cargo.toml at ${root}`);
  }
  if (!existsSync(resolve(webDir, "package.json"))) {
    throw new Error(`expected web package at ${webDir}`);
  }
}

function spawnInherited(command, args, cwd) {
  const child = spawn(command, args, {
    cwd,
    env: process.env,
    stdio: "inherit",
  });
  child.on("error", (error) => {
    logError(`${command} failed to start: ${error.message}`);
  });
  return child;
}

function waitForExit(child, timeoutMs) {
  if (!child || child.exitCode !== null || child.signalCode) {
    return Promise.resolve();
  }
  return new Promise((resolveWait) => {
    const onExit = () => {
      clearTimeout(timer);
      resolveWait();
    };
    const timer = setTimeout(() => {
      child.off("exit", onExit);
      resolveWait();
    }, timeoutMs);
    child.once("exit", onExit);
  });
}

function listChildPids(pid) {
  if (process.platform === "win32") {
    return [];
  }
  try {
    const output = spawnSync("pgrep", ["-P", String(pid)], {
      encoding: "utf8",
      timeout: 1_000,
    });
    if (output.status !== 0 || !output.stdout) {
      return [];
    }
    return output.stdout
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => Number(line))
      .filter((value) => Number.isInteger(value) && value > 0);
  } catch {
    return [];
  }
}

function signalTree(pid, signal) {
  if (process.platform === "win32") {
    spawn("taskkill", ["/pid", String(pid), "/T", "/F"], { stdio: "ignore" });
    return;
  }
  for (const childPid of listChildPids(pid)) {
    signalTree(childPid, signal);
  }
  try {
    process.kill(pid, signal);
  } catch {
    // already gone
  }
}

async function stopProcess(child, label) {
  if (!child || child.exitCode !== null || child.signalCode) {
    return;
  }
  const pid = child.pid;
  if (!pid) {
    return;
  }
  log(`stopping ${label} (pid ${pid})`);
  try {
    signalTree(pid, "SIGTERM");
  } catch (error) {
    logWarn(`failed to signal ${label}: ${error instanceof Error ? error.message : error}`);
  }

  await waitForExit(child, stopWaitMs);
  if (child.exitCode === null && !child.signalCode) {
    logWarn(`${label} did not exit in time, forcing`);
    try {
      signalTree(pid, "SIGKILL");
    } catch {
      // already gone
    }
    await waitForExit(child, 2_000);
  }
}

function startBackend() {
  const generation = ++restartGeneration;
  log(`${color.bold("backend")} cargo ${backendArgs.join(" ")}`);
  const child = spawnInherited("cargo", backendArgs, root);
  backend = child;
  child.on("exit", (code, signal) => {
    if (backend !== child) {
      return;
    }
    backend = null;
    if (shuttingDown || generation !== restartGeneration) {
      return;
    }
    if (signal) {
      logWarn(`backend exited by signal ${signal}`);
    } else if (code === 0) {
      log("backend exited");
    } else {
      logWarn(`backend exited with code ${code ?? "unknown"} — waiting for the next file change`);
    }
  });
}

async function restartBackend(reason) {
  if (shuttingDown) {
    return;
  }
  log(`backend reload: ${reason}`);
  const current = backend;
  backend = null;
  restartGeneration += 1;
  await stopProcess(current, "backend");
  if (shuttingDown) {
    return;
  }
  // Brief gap so the instance lock and listen socket are released.
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 150));
  startBackend();
}

function scheduleBackendRestart(reason) {
  if (shuttingDown) {
    return;
  }
  if (restartTimer) {
    clearTimeout(restartTimer);
  }
  restartTimer = setTimeout(() => {
    restartTimer = null;
    void restartBackend(reason);
  }, debounceMs);
}

function shouldWatchPath(relativePath) {
  if (!relativePath || relativePath === ".") {
    return false;
  }
  const normalized = relativePath.replaceAll("\\", "/");
  if (
    normalized.startsWith("target/")
    || normalized.startsWith("data/")
    || normalized.startsWith("web/")
    || normalized.startsWith("app/any2api/web-assets/")
    || normalized.includes("/target/")
    || normalized.includes("/.git/")
  ) {
    return false;
  }
  const base = normalized.split("/").at(-1) ?? normalized;
  if (base.startsWith(".")) {
    return false;
  }
  if (base === "Cargo.toml" || base === "Cargo.lock" || base === "rust-toolchain.toml") {
    return true;
  }
  const dot = base.lastIndexOf(".");
  if (dot < 0) {
    return false;
  }
  return restartExtensions.has(base.slice(dot));
}

function startWatchers() {
  /** @type {import('node:fs').FSWatcher[]} */
  const watchers = [];
  for (const entry of watchRoots) {
    const full = resolve(root, entry);
    if (!existsSync(full)) {
      continue;
    }
    try {
      const watcher = watch(full, { recursive: true }, (_event, filename) => {
        const rel = filename
          ? relative(root, resolve(full, filename.toString())).replaceAll("\\", "/")
          : entry;
        if (!shouldWatchPath(rel) && !shouldWatchPath(entry)) {
          return;
        }
        scheduleBackendRestart(rel || entry);
      });
      watcher.on("error", (error) => {
        logWarn(`watch error on ${entry}: ${error.message}`);
      });
      watchers.push(watcher);
    } catch (error) {
      logWarn(`cannot watch ${entry}: ${error instanceof Error ? error.message : error}`);
    }
  }
  return watchers;
}

function startFrontend() {
  log(`${color.bold("frontend")} pnpm dev:server (Vite HMR)`);
  // Host/port/proxy come from web/vite.config.ts so HMR and /api proxy stay aligned.
  const child = spawnInherited(
    process.platform === "win32" ? "pnpm.cmd" : "pnpm",
    ["dev:server"],
    webDir,
  );
  frontend = child;
  child.on("exit", (code, signal) => {
    if (frontend !== child) {
      return;
    }
    frontend = null;
    if (shuttingDown) {
      return;
    }
    logError(
      `frontend exited (${signal ?? code ?? "unknown"}); stop with Ctrl+C or restart dev`,
    );
  });
}

async function shutdown(signal) {
  if (shuttingDown) {
    return;
  }
  shuttingDown = true;
  if (restartTimer) {
    clearTimeout(restartTimer);
    restartTimer = null;
  }
  log(`shutting down (${signal})`);
  await Promise.all([
    stopProcess(frontend, "frontend"),
    stopProcess(backend, "backend"),
  ]);
  frontend = null;
  backend = null;
  process.exit(0);
}

async function main() {
  ensureLayout();
  const apiBind = process.env.ANY2API_BIND?.trim() || "127.0.0.1:3210";
  const uiOrigin = process.env.ANY2API_DEV_UI || "http://127.0.0.1:5173";
  const apiOrigin = apiBind.includes("://") ? apiBind : `http://${apiBind}`;

  log(color.bold("any2api development mode"));
  log(`workspace ${color.dim(root)}`);
  log(`UI  ${color.green(uiOrigin)}  ${color.dim("(Vite HMR — open this in the browser)")}`);
  log(`API ${color.green(apiOrigin)}  ${color.dim("(auto-restarts on Rust changes)")}`);
  log(color.dim("Rust/migration edits restart the backend; web/src edits hot-refresh in the browser."));

  const watchers = startWatchers();
  startBackend();
  startFrontend();

  const onSignal = (signal) => {
    for (const watcher of watchers) {
      watcher.close();
    }
    void shutdown(signal);
  };
  process.on("SIGINT", () => onSignal("SIGINT"));
  process.on("SIGTERM", () => onSignal("SIGTERM"));
}

main().catch((error) => {
  logError(error instanceof Error ? error.stack ?? error.message : String(error));
  process.exit(1);
});
