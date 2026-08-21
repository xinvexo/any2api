import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";

import { cargoTargetDirectory } from "./cargo.mjs";
import { pnpmCommand, repositoryRoot, webRoot } from "./paths.mjs";
import { runCommand } from "./process.mjs";

export const WEB_ASSET_DIR_ENV = "ANY2API_BUILD_WEB_DIR";

export async function prepareWebAssets({ targetDirectory, onProcess, grouped = true } = {}) {
  const cargoTarget = targetDirectory ?? (await cargoTargetDirectory({ onProcess }));
  const stagingRoot = join(cargoTarget, "any2api", "web-assets");
  await mkdir(stagingRoot, { recursive: true });
  const staging = await mkdtemp(join(stagingRoot, "build-"));
  const assetRoot = join(staging, "root");

  try {
    await runCommand(pnpmCommand, ["--dir", "web", "build"], {
      cwd: repositoryRoot,
      env: { ...process.env, ANY2API_WEB_OUTPUT_DIR: assetRoot },
      label: "build Web application",
      grouped,
      onProcess,
    });
    return {
      assetRoot,
      cleanup: () => rm(staging, { recursive: true, force: true }),
    };
  } catch (error) {
    await rm(staging, { recursive: true, force: true });
    throw error;
  }
}
