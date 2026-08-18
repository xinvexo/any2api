import { join } from "node:path";

import { buildApplication } from "./build.mjs";
import { repositoryRoot } from "./paths.mjs";
import {
  LifecycleController,
  LifecycleInterrupted,
  ManagedProcess,
} from "./process.mjs";

const lifecycle = new LifecycleController().install();

try {
  const build = await buildApplication({
    profile: "debug",
    onProcess: lifecycle.onProcess,
    throwIfInterrupted: () => lifecycle.throwIfInterrupted(),
  });
  lifecycle.throwIfInterrupted();
  const child = ManagedProcess.start(
    process.execPath,
    [join("web", "e2e", "run-playwright.mjs")],
    {
      cwd: repositoryRoot,
      env: { ...process.env, ANY2API_E2E_BINARY: build.executable },
      grouped: true,
      label: "Playwright E2E",
    },
  );
  lifecycle.onProcess(child);
  const result = await child.wait();
  await child.stop({ graceMs: 1_000 });
  lifecycle.onProcess(undefined);
  lifecycle.throwIfInterrupted();
  if (result.code !== 0) process.exitCode = result.code ?? 1;
} catch (error) {
  if (error instanceof LifecycleInterrupted || lifecycle.signal) {
    process.exitCode = 130;
  } else {
    throw error;
  }
} finally {
  await lifecycle.stopAll({ graceMs: 0 });
  lifecycle.dispose();
}
