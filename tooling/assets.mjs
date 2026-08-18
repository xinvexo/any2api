import { createHash } from "node:crypto";
import {
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { join, relative, sep } from "node:path";

import { cargoTargetDirectory } from "./cargo.mjs";
import { pnpmCommand, repositoryRoot, webRoot } from "./paths.mjs";
import { runCommand } from "./process.mjs";

export const ASSET_MANIFEST_ENV = "ANY2API_BUILD_WEB_ASSET_MANIFEST";
export const ASSET_MANIFEST_SCHEMA = 1;

export async function prepareWebAssets({ targetDirectory, onProcess, grouped = true } = {}) {
  const cargoTarget = targetDirectory ?? (await cargoTargetDirectory({ onProcess }));
  const publicationRoot = join(cargoTarget, "any2api", "web-assets");
  await mkdir(publicationRoot, { recursive: true });
  const staging = await mkdtemp(join(publicationRoot, ".staging-"));
  const assetRoot = join(staging, "root");

  try {
    await runCommand(pnpmCommand, ["--dir", "web", "build"], {
      cwd: repositoryRoot,
      env: { ...process.env, ANY2API_WEB_OUTPUT_DIR: assetRoot },
      label: "build Web application",
      grouped,
      onProcess,
    });
    const manifest = await createAssetManifest(assetRoot);
    await writeFile(
      join(staging, "manifest.json"),
      `${JSON.stringify(manifest, null, 2)}\n`,
      "utf8",
    );
    const published = join(publicationRoot, manifest.bundle_sha256);
    const manifestPath = join(published, "manifest.json");
    try {
      await rename(staging, published);
    } catch (error) {
      if (!["EEXIST", "ENOTEMPTY", "EPERM"].includes(error.code)) throw error;
      await rm(staging, { recursive: true, force: true });
    }
    return { manifest, manifestPath, assetRoot: join(published, "root") };
  } catch (error) {
    await rm(staging, { recursive: true, force: true });
    throw error;
  }
}

export async function createAssetManifest(assetRoot) {
  const files = await inspectAssetDirectory(assetRoot);
  const index = files.find((file) => file.path === "index.html");
  if (!index || index.size === 0) {
    throw new Error("Web asset output must contain a non-empty index.html");
  }
  return {
    schema: ASSET_MANIFEST_SCHEMA,
    asset_root: "root",
    bundle_sha256: bundleDigest(files),
    files,
  };
}

export async function inspectAssetDirectory(root) {
  const rootMetadata = await lstat(root);
  if (!rootMetadata.isDirectory() || rootMetadata.isSymbolicLink()) {
    throw new Error(`Web asset root is not a regular directory: ${root}`);
  }
  const paths = [];
  await walk(root, root, paths);
  paths.sort((left, right) => (left < right ? -1 : left > right ? 1 : 0));
  const files = [];
  for (const path of paths) {
    const bytes = await readFile(path);
    files.push({
      path: relative(root, path).split(sep).join("/"),
      size: bytes.byteLength,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    });
  }
  return files;
}

export function bundleDigest(files) {
  const hash = createHash("sha256");
  for (const file of files) {
    hash.update(file.path, "utf8");
    hash.update("\0");
    hash.update(String(file.size), "utf8");
    hash.update("\0");
    hash.update(file.sha256, "utf8");
    hash.update("\n");
  }
  return hash.digest("hex");
}

async function walk(root, directory, paths) {
  const entries = await readdir(directory, { withFileTypes: true });
  entries.sort((left, right) => left.name.localeCompare(right.name, "en"));
  for (const entry of entries) {
    const path = join(directory, entry.name);
    const metadata = await lstat(path);
    if (metadata.isSymbolicLink()) {
      throw new Error(`Web assets cannot contain symbolic links: ${path}`);
    }
    if (metadata.isDirectory()) {
      await walk(root, path, paths);
    } else if (metadata.isFile()) {
      paths.push(path);
    } else {
      throw new Error(`Web assets must be regular files: ${path}`);
    }
  }
}
