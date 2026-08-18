import { ASSET_MANIFEST_ENV, prepareWebAssets } from "./assets.mjs";
import { cargoBuild, readBinaryVersion, resolveBuildTarget } from "./cargo.mjs";
import { generateAdminBindings } from "./bindings.mjs";

export async function buildApplication({
  target,
  targetDescriptor,
  profile = "release",
  onProcess,
  throwIfInterrupted = () => undefined,
} = {}) {
  const descriptor = targetDescriptor ?? (await resolveBuildTarget(target, { onProcess }));
  throwIfInterrupted();
  process.stdout.write("==> generate admin TypeScript bindings\n");
  const bindings = await generateAdminBindings({ onProcess });
  throwIfInterrupted();
  process.stdout.write("==> build and publish Web assets\n");
  const assets = await prepareWebAssets({ onProcess });
  throwIfInterrupted();
  process.stdout.write(`==> build Rust application for ${descriptor.targetTriple}\n`);
  const executable = await cargoBuild({
    target: descriptor.targetTriple,
    profile,
    env: {
      ...process.env,
      [ASSET_MANIFEST_ENV]: assets.manifestPath,
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
  return { assets, bindings, compiledVersion, descriptor, executable, profile };
}
