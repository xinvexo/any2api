import { createInterface } from "node:readline";

import { cargoCommand, repositoryRoot, rustcCommand } from "./paths.mjs";
import { CommandFailure, ManagedProcess, runCaptured } from "./process.mjs";

const distributionTargets = new Map([
  ["x86_64-unknown-linux-gnu", { label: "linux-amd64", archive: "tar.gz" }],
  ["aarch64-unknown-linux-gnu", { label: "linux-arm64", archive: "tar.gz" }],
  ["aarch64-apple-darwin", { label: "macos-arm64", archive: "tar.gz" }],
  ["x86_64-apple-darwin", { label: "macos-amd64", archive: "tar.gz" }],
  ["x86_64-pc-windows-msvc", { label: "windows-amd64", archive: "tar.gz" }],
  ["x86_64-pc-windows-gnu", { label: "windows-amd64", archive: "tar.gz" }],
  ["aarch64-pc-windows-msvc", { label: "windows-arm64", archive: "tar.gz" }],
]);

export async function resolveBuildTarget(explicitTarget, { onProcess } = {}) {
  const { stdout } = await runCaptured(rustcCommand, ["-vV"], {
    cwd: repositoryRoot,
    label: "read Rust host target",
    onProcess,
  });
  const hostLine = stdout.split(/\r?\n/u).find((line) => line.startsWith("host: "));
  if (!hostLine) throw new Error("rustc -vV did not report a host target");
  const hostTriple = hostLine.slice("host: ".length).trim();
  const targetTriple = explicitTarget ?? hostTriple;
  return {
    hostTriple,
    targetTriple,
    native: targetTriple === hostTriple,
  };
}

export function distributionForTarget(targetTriple) {
  const target = distributionTargets.get(targetTriple);
  if (!target) {
    throw new Error(
      `target ${targetTriple} has no distribution layout; add an explicit target mapping`,
    );
  }
  return target;
}

export async function cargoTargetDirectory({ onProcess } = {}) {
  const { stdout } = await runCaptured(
    cargoCommand,
    ["metadata", "--locked", "--format-version=1", "--no-deps"],
    { cwd: repositoryRoot, label: "cargo metadata", onProcess },
  );
  const metadata = JSON.parse(stdout);
  if (typeof metadata.target_directory !== "string") {
    throw new Error("cargo metadata did not report target_directory");
  }
  return metadata.target_directory;
}

export async function cargoBuild({
  target,
  profile = "release",
  env = process.env,
  onProcess,
  grouped = true,
}) {
  const args = [
    "build",
    "--locked",
    "--package",
    "any2api",
    "--message-format=json-render-diagnostics",
  ];
  if (profile === "release") args.push("--release");
  else if (profile !== "debug") throw new Error(`unsupported Cargo profile ${profile}`);
  if (target) args.push("--target", target);

  const label = `cargo build (${profile}, ${target ?? "host"})`;
  const child = ManagedProcess.start(cargoCommand, args, {
    cwd: repositoryRoot,
    env,
    label,
    grouped,
    stdio: ["ignore", "pipe", "inherit"],
  });
  onProcess?.(child);
  child.child.stdout.setEncoding("utf8");
  let executable;
  const lines = createInterface({ input: child.child.stdout, crlfDelay: Infinity });
  for await (const line of lines) {
    if (!line) continue;
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      process.stdout.write(`${line}\n`);
      continue;
    }
    if (message.reason === "compiler-message" && message.message?.rendered) {
      process.stderr.write(message.message.rendered);
    }
    if (
      message.reason === "compiler-artifact" &&
      message.target?.name === "any2api" &&
      message.target?.kind?.includes("bin") &&
      typeof message.executable === "string"
    ) {
      executable = message.executable;
    }
  }
  let result;
  try {
    result = await child.wait();
    if (child.grouped) await child.stop({ graceMs: 1_000 });
  } finally {
    onProcess?.(undefined);
  }
  if (result.code !== 0) throw new CommandFailure(label, result);
  if (!executable) throw new Error("Cargo did not report the any2api executable artifact");
  return executable;
}

export async function readBinaryVersion(executable, { onProcess } = {}) {
  const { stdout } = await runCaptured(executable, ["--version"], {
    cwd: repositoryRoot,
    label: `${executable} --version`,
    onProcess,
  });
  const match = /^any2api (\S+)\s*$/u.exec(stdout);
  if (!match) throw new Error(`unexpected any2api --version output: ${JSON.stringify(stdout)}`);
  return match[1];
}
