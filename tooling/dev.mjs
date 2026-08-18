import { copyFile, chmod, mkdtemp, rm } from "node:fs/promises";
import { rmSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, extname, join, relative, resolve } from "node:path";

import chokidar from "chokidar";

import { ASSET_MANIFEST_ENV } from "./assets.mjs";
import { generateAdminBindings } from "./bindings.mjs";
import { cargoBuild, resolveBuildTarget } from "./cargo.mjs";
import { binaryName, repositoryRoot, webRoot } from "./paths.mjs";
import { ManagedProcess } from "./process.mjs";
import { BackendBuildCoordinator } from "./dev/backend-coordinator.mjs";

const defaultBind = "127.0.0.1:3210";

export async function runDevelopment() {
  const sessionDirectory = await mkdtemp(join(tmpdir(), "any2api-dev-"));
  let descriptor;
  const childEnvironment = { ...process.env };
  delete childEnvironment[ASSET_MANIFEST_ENV];
  let frontend;
  let runtime;
  let activeBuild;
  let watcher;
  let coordinator;
  let shutdownPromise;
  let resolveSession;
  const session = new Promise((resolveSessionPromise) => {
    resolveSession = resolveSessionPromise;
  });
  const exitCleanup = () => {
    coordinator?.stop();
    for (const processHandle of [activeBuild, runtime, frontend]) {
      processHandle?.terminateNow();
    }
    try {
      rmSync(sessionDirectory, { recursive: true, force: true });
    } catch {
      // Best-effort final cleanup; process groups are the correctness boundary.
    }
  };
  process.once("exit", exitCleanup);
  descriptor = await resolveBuildTarget(undefined, { onProcess: setActiveBuild });

  coordinator = new BackendBuildCoordinator({
    build: async ({ bindings, epoch }) => {
      if (bindings) {
        await generateAdminBindings({ onProcess: setActiveBuild, grouped: true });
      }
      const executable = await cargoBuild({
        target: descriptor.targetTriple,
        profile: "debug",
        env: childEnvironment,
        onProcess: setActiveBuild,
        grouped: true,
      });
      const staged = join(
        sessionDirectory,
        `${binaryName(descriptor.targetTriple)}-${epoch}${extname(executable)}`,
      );
      await copyFile(executable, staged);
      if (!descriptor.targetTriple.includes("-windows-")) await chmod(staged, 0o755);
      return staged;
    },
    deploy: async (executable, _epoch, isCurrent) => {
      if (shutdownPromise) return;
      const previous = runtime;
      if (previous) await previous.stop({ graceMs: 10_000 });
      if (shutdownPromise || !isCurrent()) return;
      const launched = ManagedProcess.start(executable, [], {
        cwd: repositoryRoot,
        env: childEnvironment,
        grouped: true,
        label: "any2api development backend",
      });
      runtime = launched;
      launched.closed.then((result) => {
        if (runtime === launched) runtime = undefined;
        if (!launched.expectedExit && !shutdownPromise) {
          void launched.stop({ graceMs: 1_000 });
          process.stderr.write(
            `backend exited (${result.error?.message ?? result.signal ?? result.code}); waiting for the next Rust change\n`,
          );
        }
      });
      process.stdout.write(`backend running from ${executable}\n`);
    },
    onBuildFailure: (error) => {
      process.stderr.write(`backend build failed; keeping the current runtime\n${error.stack ?? error}\n`);
    },
  });

  function setActiveBuild(processHandle) {
    activeBuild = processHandle;
  }

  async function shutdown(exitCode = 0, { force = false } = {}) {
    if (shutdownPromise) return shutdownPromise;
    shutdownPromise = (async () => {
      coordinator.stop();
      await watcher?.close();
      await activeBuild?.stop({ graceMs: force ? 0 : 5_000 });
      await coordinator.waitForIdle();
      await Promise.all([
        runtime?.stop({ graceMs: force ? 0 : 10_000 }),
        frontend?.stop({ graceMs: force ? 0 : 5_000 }),
      ]);
      await rm(sessionDirectory, { recursive: true, force: true });
      if (exitCode !== 0) process.exitCode = exitCode;
      resolveSession();
    })();
    return shutdownPromise;
  }

  function emergencyStop() {
    coordinator.stop();
    for (const processHandle of [activeBuild, runtime, frontend]) {
      processHandle?.terminateNow();
    }
    try {
      rmSync(sessionDirectory, { recursive: true, force: true });
    } catch {
      // The asynchronous shutdown path retries cleanup when the parent remains alive.
    }
  }

  const onSignal = () => {
    process.stderr.write("stopping development processes\n");
    emergencyStop();
    void shutdown(0, { force: true });
  };
  process.once("SIGINT", onSignal);
  process.once("SIGTERM", onSignal);

  try {
    frontend = startVite(true);
    frontend.closed.then((result) => {
      if (!frontend.expectedExit && !shutdownPromise) {
        process.stderr.write(
          `Vite exited unexpectedly (${result.error?.message ?? result.signal ?? result.code})\n`,
        );
        void shutdown(1);
      }
    });

    watcher = chokidar.watch(rustWatchInputs(), {
      ignoreInitial: true,
      awaitWriteFinish: { stabilityThreshold: 100, pollInterval: 25 },
    });
    watcher.on("all", (_event, path) => {
      coordinator.request({ bindings: isBindingInput(resolve(path)) });
    });
    const watcherReady = new Promise((resolveReady, rejectReady) => {
      watcher.once("ready", resolveReady);
      watcher.once("error", rejectReady);
    });
    const startup = await Promise.race([
      watcherReady.then(() => "ready"),
      session.then(() => "shutdown"),
    ]);
    if (startup === "shutdown") return;
    watcher.on("error", (error) => {
      process.stderr.write(`Rust source watcher failed: ${error.stack ?? error}\n`);
      void shutdown(1);
    });
    coordinator.request({ bindings: true, immediate: true });
    process.stdout.write(
      `development frontend http://127.0.0.1:5173, API ${apiTarget(process.env.ANY2API_BIND)}\n`,
    );
    await session;
  } catch (error) {
    await shutdown(1);
    throw error;
  } finally {
    process.removeListener("SIGINT", onSignal);
    process.removeListener("SIGTERM", onSignal);
    process.removeListener("exit", exitCleanup);
  }
}

function startVite(grouped) {
  const require = createRequire(join(webRoot, "package.json"));
  const vitePackage = require.resolve("vite/package.json");
  const viteEntry = join(dirname(vitePackage), "bin", "vite.js");
  return ManagedProcess.start(process.execPath, [viteEntry], {
    cwd: webRoot,
    env: {
      ...process.env,
      VITE_API_TARGET: process.env.VITE_API_TARGET || apiTarget(process.env.ANY2API_BIND),
    },
    grouped,
    label: "Vite development server",
  });
}

function rustWatchInputs() {
  return [
    join(repositoryRoot, "app"),
    join(repositoryRoot, "crates"),
    join(repositoryRoot, "migrations"),
    join(repositoryRoot, "Cargo.toml"),
    join(repositoryRoot, "Cargo.lock"),
    join(repositoryRoot, "rust-toolchain.toml"),
    join(repositoryRoot, ".cargo", "config.toml"),
  ];
}

export function isBindingInput(path) {
  const relativePath = relative(repositoryRoot, path).split("\\").join("/");
  return (
    relativePath.startsWith("crates/server/src/admin/") ||
    relativePath === "crates/server/Cargo.toml" ||
    relativePath === "Cargo.toml" ||
    relativePath === "Cargo.lock" ||
    relativePath === ".cargo/config.toml"
  );
}

export function apiTarget(bind = defaultBind) {
  const value = bind || defaultBind;
  if (value.startsWith("[")) {
    const end = value.indexOf("]");
    if (end < 0 || value[end + 1] !== ":") throw new Error(`invalid ANY2API_BIND ${value}`);
    const host = value.slice(1, end) === "::" ? "::1" : value.slice(1, end);
    return `http://[${host}]:${value.slice(end + 2)}`;
  }
  const separator = value.lastIndexOf(":");
  if (separator <= 0) throw new Error(`invalid ANY2API_BIND ${value}`);
  const rawHost = value.slice(0, separator);
  const host = rawHost === "0.0.0.0" ? "127.0.0.1" : rawHost;
  return `http://${host}:${value.slice(separator + 1)}`;
}
