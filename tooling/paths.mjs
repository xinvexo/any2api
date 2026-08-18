import { fileURLToPath } from "node:url";
import { join } from "node:path";

export const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
export const webRoot = join(repositoryRoot, "web");
export const generatedBindingsRoot = join(
  webRoot,
  "src",
  "shared",
  "api",
  "generated",
);
export const distributionRoot = join(repositoryRoot, "dist");

export const cargoCommand = process.env.CARGO || "cargo";
export const rustcCommand = process.env.RUSTC || "rustc";
export const pnpmCommand = process.platform === "win32" ? "pnpm.cmd" : "pnpm";

export function binaryName(targetTriple) {
  return targetTriple.includes("-windows-") ? "any2api.exe" : "any2api";
}
