import { randomUUID } from "node:crypto";
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
import { dirname, join, relative, sep } from "node:path";

import { cargoTargetDirectory } from "./cargo.mjs";
import { cargoCommand, generatedBindingsRoot, repositoryRoot } from "./paths.mjs";
import { runCommand } from "./process.mjs";

export async function generateAdminBindings({ onProcess, grouped = true } = {}) {
  const targetDirectory = await cargoTargetDirectory({ onProcess });
  const scratchRoot = join(targetDirectory, "any2api", "bindings");
  await mkdir(scratchRoot, { recursive: true });
  const temporary = await mkdtemp(join(scratchRoot, "export-"));
  try {
    await runCommand(
      cargoCommand,
      [
        "test",
        "--locked",
        "--package",
        "any2api-server",
        "export_admin_bindings",
        "--",
        "--ignored",
        "--test-threads=1",
      ],
      {
        cwd: repositoryRoot,
        env: {
          ...process.env,
          TS_RS_EXPORT_DIR: temporary,
        },
        label: "generate admin TypeScript bindings",
        grouped,
        onProcess,
      },
    );
    const generated = await listRegularFiles(temporary);
    if (generated.length === 0 || generated.some((path) => !path.endsWith(".ts"))) {
      throw new Error("admin binding exporter did not produce a non-empty TypeScript file set");
    }
    const changes = await exactSync(temporary, generatedBindingsRoot, generated);
    if (changes.length > 0) {
      process.stdout.write(`updated admin bindings: ${changes.join(", ")}\n`);
    }
    return { files: generated, changes };
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
}

export async function exactSync(sourceRoot, destinationRoot, sourceFiles) {
  await mkdir(destinationRoot, { recursive: true });
  const destinationFiles = await listRegularFiles(destinationRoot);
  const sourceSet = new Set(sourceFiles);
  const changes = [];

  for (const path of destinationFiles) {
    if (sourceSet.has(path)) continue;
    await rm(join(destinationRoot, path));
    changes.push(`-${path}`);
  }
  for (const path of sourceFiles) {
    const source = await readFile(join(sourceRoot, path));
    const destination = join(destinationRoot, path);
    let current;
    try {
      current = await readFile(destination);
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    if (current?.equals(source)) continue;
    await mkdir(dirname(destination), { recursive: true });
    const temporary = `${destination}.tmp-${process.pid}-${randomUUID()}`;
    await writeFile(temporary, source);
    try {
      await rename(temporary, destination);
    } catch (error) {
      if (!["EEXIST", "EPERM"].includes(error.code)) throw error;
      await rm(destination, { force: true });
      await rename(temporary, destination);
    }
    changes.push(`+${path}`);
  }
  await removeEmptyDirectories(destinationRoot, destinationRoot);
  return changes;
}

export async function listRegularFiles(root) {
  let metadata;
  try {
    metadata = await lstat(root);
  } catch (error) {
    if (error.code === "ENOENT") return [];
    throw error;
  }
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error(`generated file root is not a regular directory: ${root}`);
  }
  const files = [];
  await walk(root, root, files);
  files.sort((left, right) => left.localeCompare(right, "en"));
  return files;
}

async function walk(root, directory, files) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(directory, entry.name);
    const metadata = await lstat(path);
    if (metadata.isSymbolicLink()) {
      throw new Error(`generated files cannot contain symbolic links: ${path}`);
    }
    if (metadata.isDirectory()) await walk(root, path, files);
    else if (metadata.isFile()) files.push(relative(root, path).split(sep).join("/"));
    else throw new Error(`generated output must contain regular files: ${path}`);
  }
}

async function removeEmptyDirectories(root, directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    await removeEmptyDirectories(root, join(directory, entry.name));
  }
  if (directory !== root && (await readdir(directory)).length === 0) await rm(directory);
}
