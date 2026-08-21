import { createHash, randomUUID } from "node:crypto";
import {
  chmod,
  copyFile,
  mkdir,
  mkdtemp,
  readFile,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { basename, join } from "node:path";

import { create as createTar } from "tar";

import { buildApplication } from "./build.mjs";
import { distributionForTarget, resolveBuildTarget } from "./cargo.mjs";
import { binaryName, distributionRoot } from "./paths.mjs";

export async function packageApplication({ target, onProcess, throwIfInterrupted = () => undefined } = {}) {
  const descriptor = await resolveBuildTarget(target, { onProcess });
  const distribution = distributionForTarget(descriptor.targetTriple);
  if (!descriptor.native && !process.env.ANY2API_BUILD_VERSION) {
    throw new Error("cross-target packaging requires ANY2API_BUILD_VERSION");
  }
  const build = await buildApplication({
    targetDescriptor: descriptor,
    profile: "release",
    onProcess,
    throwIfInterrupted,
  });
  throwIfInterrupted();
  const version = build.compiledVersion ?? process.env.ANY2API_BUILD_VERSION;
  validateArtifactVersion(version);

  const targetDirectory = build.targetDirectory;
  const scratchRoot = join(targetDirectory, "any2api", "package");
  await mkdir(scratchRoot, { recursive: true });
  const staging = await mkdtemp(join(scratchRoot, "staging-"));
  const name = binaryName(descriptor.targetTriple);
  const stagedBinary = join(staging, name);
  const archiveName = `any2api-v${version}-${distribution.label}.${distribution.archive}`;
  await mkdir(distributionRoot, { recursive: true });
  const temporaryArchive = join(
    distributionRoot,
    `.${archiveName}.${randomUUID()}.tmp`,
  );
  let checksumTemporary;

  try {
    throwIfInterrupted();
    await copyFile(build.executable, stagedBinary);
    if (!descriptor.targetTriple.includes("-windows-")) await chmod(stagedBinary, 0o755);
    throwIfInterrupted();
    await createTar(
      {
        cwd: staging,
        file: temporaryArchive,
        gzip: true,
        noMtime: true,
        portable: true,
      },
      [name],
    );
    throwIfInterrupted();
    const archiveBytes = await readFile(temporaryArchive);
    const checksum = createHash("sha256").update(archiveBytes).digest("hex");

    const archivePath = join(distributionRoot, archiveName);
    const checksumPath = `${archivePath}.sha256`;
    throwIfInterrupted();
    await publishFile(temporaryArchive, archivePath);
    checksumTemporary = join(
      distributionRoot,
      `.${basename(checksumPath)}.${randomUUID()}.tmp`,
    );
    await writeFile(checksumTemporary, `${checksum}  ${archiveName}\n`, "utf8");
    throwIfInterrupted();
    await publishFile(checksumTemporary, checksumPath);
    return { ...build, archivePath, checksum, checksumPath, version };
  } finally {
    await rm(temporaryArchive, { force: true });
    if (checksumTemporary) await rm(checksumTemporary, { force: true });
    await rm(staging, { recursive: true, force: true });
  }
}

function validateArtifactVersion(version) {
  if (!version || !/^[0-9A-Za-z][0-9A-Za-z.+-]*$/u.test(version)) {
    throw new Error(`invalid compiled product version ${JSON.stringify(version)}`);
  }
}

async function publishFile(source, destination) {
  try {
    await rename(source, destination);
  } catch (error) {
    if (!["EEXIST", "EPERM"].includes(error.code)) throw error;
    await rm(destination, { force: true });
    await rename(source, destination);
  }
}
