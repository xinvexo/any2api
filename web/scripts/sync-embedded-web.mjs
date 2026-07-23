import { cp, lstat, mkdir, readFile, readdir, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { join, relative, resolve } from "node:path";

const webDirectory = resolve(fileURLToPath(new URL("..", import.meta.url)));
const repository = resolve(webDirectory, "..");
const source = join(webDirectory, "dist");
const target = join(repository, "app", "any2api", "web-assets");

export async function syncEmbeddedWeb() {
  const sourceFiles = await files(source);
  if (!sourceFiles.includes("index.html")) {
    throw new Error("web/dist must contain index.html; run pnpm build first");
  }

  await rm(target, { recursive: true, force: true });
  await mkdir(target, { recursive: true });
  await cp(source, target, { recursive: true });
  console.log(`synchronized ${sourceFiles.length} embedded web assets`);
  return sourceFiles;
}

export async function checkEmbeddedWeb() {
  const sourceFiles = await files(source);
  if (!sourceFiles.includes("index.html")) {
    throw new Error("web/dist must contain index.html; run pnpm build first");
  }
  await check(sourceFiles);
  console.log(`embedded web assets are current (${sourceFiles.length} files)`);
  return sourceFiles;
}

async function check(expectedFiles) {
  const actualFiles = await files(target, true);
  if (JSON.stringify(actualFiles) !== JSON.stringify(expectedFiles)) {
    throw new Error("embedded web asset file list is stale; run pnpm build:embedded");
  }
  for (const path of expectedFiles) {
    const [expected, actual] = await Promise.all([
      readFile(join(source, path)),
      readFile(join(target, path)),
    ]);
    if (!expected.equals(actual)) {
      throw new Error(`embedded web asset ${path} is stale; run pnpm build:embedded`);
    }
  }
}

async function files(root, missingIsEmpty = false) {
  const output = [];
  try {
    const rootStat = await lstat(root);
    if (rootStat.isSymbolicLink() || !rootStat.isDirectory()) {
      throw new Error(`embedded web asset root must be a regular directory: ${root}`);
    }
    await visit(root, root, output);
  } catch (error) {
    if (missingIsEmpty && error?.code === "ENOENT") return output;
    throw error;
  }
  return output.sort();
}

async function visit(root, directory, output) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      throw new Error(`embedded web assets cannot contain symbolic links: ${path}`);
    } else if (entry.isDirectory()) {
      await visit(root, path, output);
    } else if (entry.isFile()) {
      output.push(relative(root, path).replaceAll("\\", "/"));
    } else {
      throw new Error(`embedded web assets must be regular files: ${path}`);
    }
  }
}

const isCli = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isCli) {
  const checkOnly = process.argv.includes("--check");
  try {
    if (checkOnly) {
      await checkEmbeddedWeb();
    } else {
      await syncEmbeddedWeb();
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exitCode = 1;
  }
}
