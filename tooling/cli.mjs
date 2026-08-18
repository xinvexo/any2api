import { buildApplication } from "./build.mjs";
import { packageApplication } from "./package.mjs";
import { LifecycleController, LifecycleInterrupted } from "./process.mjs";

const [command, ...arguments_] = process.argv.slice(2);
const lifecycle = command === "build" || command === "package"
  ? new LifecycleController().install()
  : undefined;

try {
  if (command === "dev") {
    rejectArguments(arguments_, "dev");
    const { runDevelopment } = await import("./dev.mjs");
    await runDevelopment();
  } else if (command === "build") {
    const options = parseTarget(arguments_, "build");
    const result = await buildApplication({
      ...options,
      onProcess: lifecycle.onProcess,
      throwIfInterrupted: () => lifecycle.throwIfInterrupted(),
    });
    process.stdout.write(`built ${result.executable}\n`);
  } else if (command === "package") {
    const options = parseTarget(arguments_, "package");
    const result = await packageApplication({
      ...options,
      onProcess: lifecycle.onProcess,
      throwIfInterrupted: () => lifecycle.throwIfInterrupted(),
    });
    process.stdout.write(`packaged ${result.archivePath}\nchecksum ${result.checksumPath}\n`);
  } else if (command === "help" || command === "--help" || command === "-h") {
    printUsage();
  } else {
    throw new Error(`unknown lifecycle command ${JSON.stringify(command)}`);
  }
} catch (error) {
  if (error instanceof LifecycleInterrupted || lifecycle?.signal) {
    process.exitCode = 130;
  } else {
    process.stderr.write(`${error.stack ?? error}\n`);
    process.exitCode = 1;
  }
} finally {
  lifecycle?.dispose();
}

function parseTarget(arguments_, commandName) {
  let target;
  for (let index = 0; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    let value;
    if (argument === "--target") value = arguments_[++index];
    else if (argument.startsWith("--target=")) value = argument.slice("--target=".length);
    else if (argument === "--help" || argument === "-h") {
      printUsage();
      process.exit(0);
    } else throw new Error(`unknown ${commandName} argument ${JSON.stringify(argument)}`);
    if (!value || value.startsWith("-")) throw new Error("--target requires a target triple");
    if (target) throw new Error("--target may only be specified once");
    target = value;
  }
  return { target };
}

function rejectArguments(arguments_, commandName) {
  if (arguments_.length > 0) {
    throw new Error(`${commandName} does not accept arguments: ${arguments_.join(" ")}`);
  }
}

function printUsage() {
  process.stdout.write(
    "usage: pnpm <dev|build|package> [--target <rust-target-triple>]\n",
  );
}
