import { spawn, spawnSync } from "node:child_process";

const isWindows = process.platform === "win32";

export class CommandFailure extends Error {
  constructor(label, result) {
    const status = result.error?.message ?? result.signal ?? result.code ?? "unknown status";
    super(`${label} failed with ${status}`);
    this.name = "CommandFailure";
    this.result = result;
  }
}

export class LifecycleInterrupted extends Error {
  constructor(signal) {
    super(`lifecycle interrupted by ${signal}`);
    this.name = "LifecycleInterrupted";
    this.signal = signal;
  }
}

export class LifecycleController {
  constructor() {
    this.active = new Set();
    this.signal = undefined;
    this.onSignal = (signal) => {
      if (!this.signal) this.signal = signal;
      void this.stopAll({ graceMs: 0 });
    };
    this.onExit = () => this.terminateNow();
  }

  install() {
    process.once("SIGINT", this.onSignal);
    process.once("SIGTERM", this.onSignal);
    process.once("exit", this.onExit);
    return this;
  }

  onProcess = (processHandle) => {
    if (processHandle) {
      this.active.add(processHandle);
      if (this.signal) void processHandle.stop({ graceMs: 0 });
      return;
    }
    for (const handle of this.active) {
      if (handle.result) this.active.delete(handle);
    }
  };

  throwIfInterrupted() {
    if (this.signal) throw new LifecycleInterrupted(this.signal);
  }

  async stopAll({ graceMs = 5_000 } = {}) {
    await Promise.all(
      [...this.active].map((handle) => handle.stop({ graceMs }).catch(() => undefined)),
    );
  }

  terminateNow() {
    for (const handle of this.active) handle.terminateNow();
  }

  dispose() {
    process.removeListener("SIGINT", this.onSignal);
    process.removeListener("SIGTERM", this.onSignal);
    process.removeListener("exit", this.onExit);
  }
}

export class ManagedProcess {
  static start(program, args, options = {}) {
    const grouped = options.grouped ?? false;
    const child = spawn(program, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: options.stdio ?? "inherit",
      detached: grouped && !isWindows,
      windowsHide: true,
    });
    return new ManagedProcess(child, options.label ?? program, grouped);
  }

  constructor(child, label, grouped) {
    this.child = child;
    this.label = label;
    this.grouped = grouped;
    this.expectedExit = false;
    this.result = undefined;
    this.closed = new Promise((resolve) => {
      const finish = (result) => {
        if (this.result) return;
        this.result = result;
        resolve(result);
      };
      child.once("error", (error) => finish({ code: null, signal: null, error }));
      child.once("close", (code, signal) => finish({ code, signal }));
    });
  }

  get pid() {
    return this.child.pid;
  }

  async wait() {
    return this.closed;
  }

  async stop({ graceMs = 10_000 } = {}) {
    this.expectedExit = true;
    if (!this.pid) return this.closed;
    if (isWindows) {
      if (this.grouped || !this.result) await stopWindowsTree(this, graceMs);
    } else if (this.grouped) {
      await stopPosixGroup(this, graceMs);
    } else if (!this.result) {
      this.child.kill("SIGTERM");
      if (!(await settlesWithin(this.closed, graceMs))) this.child.kill("SIGKILL");
    }
    return this.closed;
  }

  terminateNow() {
    this.expectedExit = true;
    if (!this.pid) return;
    if (isWindows && this.grouped) {
      spawnSync("taskkill", ["/PID", String(this.pid), "/T", "/F"], {
        stdio: "ignore",
        windowsHide: true,
      });
      return;
    }
    if (!isWindows && this.grouped) {
      signalPosixGroup(this.pid, "SIGKILL");
      return;
    }
    if (!this.result) this.child.kill("SIGKILL");
  }
}

export async function runCommand(program, args, options = {}) {
  const label = options.label ?? [program, ...args].join(" ");
  const child = ManagedProcess.start(program, args, {
    ...options,
    label,
    grouped: options.grouped ?? false,
  });
  options.onProcess?.(child);
  let result;
  try {
    result = await child.wait();
    if (child.grouped) await child.stop({ graceMs: 1_000 });
  } finally {
    options.onProcess?.(undefined);
  }
  if (result.code !== 0) throw new CommandFailure(label, result);
  return result;
}

export async function runCaptured(program, args, options = {}) {
  const label = options.label ?? [program, ...args].join(" ");
  const child = ManagedProcess.start(program, args, {
    cwd: options.cwd,
    env: options.env,
    label,
    stdio: ["ignore", "pipe", "pipe"],
    grouped: options.grouped ?? true,
  });
  options.onProcess?.(child);
  let stdout = "";
  let stderr = "";
  child.child.stdout.setEncoding("utf8");
  child.child.stderr.setEncoding("utf8");
  child.child.stdout.on("data", (chunk) => (stdout += chunk));
  child.child.stderr.on("data", (chunk) => (stderr += chunk));
  let result;
  try {
    result = await child.wait();
    if (child.grouped) await child.stop({ graceMs: 1_000 });
  } finally {
    options.onProcess?.(undefined);
  }
  if (result.code !== 0) {
    const failure = new CommandFailure(label, result);
    failure.stdout = stdout;
    failure.stderr = stderr;
    if (stderr.trim()) failure.message += `\n${stderr.trimEnd()}`;
    throw failure;
  }
  return { stdout, stderr };
}

async function stopPosixGroup(processHandle, graceMs) {
  signalPosixGroup(processHandle.pid, "SIGTERM");
  if (!(await waitFor(() => !posixGroupExists(processHandle.pid), graceMs))) {
    signalPosixGroup(processHandle.pid, "SIGKILL");
    const stopped = await waitFor(
      () => !posixGroupExists(processHandle.pid),
      Math.max(250, Math.min(Math.max(graceMs, 1_000), 5_000)),
    );
    if (!stopped) throw new Error(`process group ${processHandle.pid} did not stop`);
  }
}

async function stopWindowsTree(processHandle, graceMs) {
  const killer = spawn("taskkill", ["/PID", String(processHandle.pid), "/T", "/F"], {
    stdio: "ignore",
    windowsHide: true,
  });
  await new Promise((resolve) => killer.once("close", resolve));
  await settlesWithin(processHandle.closed, graceMs);
}

function signalPosixGroup(pid, signal) {
  try {
    process.kill(-pid, signal);
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
}

function posixGroupExists(pid) {
  try {
    process.kill(-pid, 0);
    return true;
  } catch (error) {
    if (error.code === "ESRCH") return false;
    if (error.code === "EPERM") return true;
    throw error;
  }
}

async function waitFor(predicate, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (!predicate()) {
    if (Date.now() >= deadline) return false;
    await delay(40);
  }
  return true;
}

async function settlesWithin(promise, timeoutMs) {
  return Promise.race([
    promise.then(() => true),
    delay(timeoutMs).then(() => false),
  ]);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
