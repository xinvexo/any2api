import { WEB_ASSET_DIR_ENV, prepareWebAssets } from "./assets.mjs";
import {
  cargoBuild,
  cargoTargetDirectory,
  readBinaryVersion,
  resolveBuildTarget,
} from "./cargo.mjs";
import { generateAdminBindings } from "./bindings.mjs";

export async function buildApplication({
  target,
  targetDescriptor,
  profile = "release",
  onProcess,
  throwIfInterrupted = () => undefined,
} = {}) {
  const descriptor = targetDescriptor ?? (await resolveBuildTarget(target, { onProcess }));
  const targetDirectory = await cargoTargetDirectory({ onProcess });
  throwIfInterrupted();
  process.stdout.write("==> generate admin TypeScript bindings\n");
  const bindings = await generateAdminBindings({ targetDirectory, onProcess });
  throwIfInterrupted();
  process.stdout.write("==> build Web assets\n");
  const assets = await prepareWebAssets({ targetDirectory, onProcess });
  try {
    throwIfInterrupted();
    process.stdout.write(`==> build Rust application for ${descriptor.targetTriple}\n`);
    const executable = await cargoBuild({
      target: descriptor.targetTriple,
      profile,
      env: {
        ...process.env,
        [WEB_ASSET_DIR_ENV]: assets.assetRoot,
      },
      onProcess,
    });
    throwIfInterrupted();

    let compiledVersion;
    if (descriptor.native) {
      compiledVersion = await readBinaryVersion(executable, { onProcess });
      throwIfInterrupted();
      const requestedVersion = process.env.ANY2API_BUILD_VERSION;
      if (requestedVersion && requestedVersion !== compiledVersion) {
        throw new Error(
          `compiled version ${compiledVersion} does not match ANY2API_BUILD_VERSION ${requestedVersion}`,
        );
      }
    }
    return { bindings, compiledVersion, descriptor, executable, profile, targetDirectory };
  } finally {
    await assets.cleanup();
  }
}
