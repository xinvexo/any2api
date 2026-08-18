import assert from "node:assert/strict";
import test from "node:test";

import { apiTarget } from "../dev.mjs";
import { ManagedProcess } from "../process.mjs";

test("development proxy follows the backend bind address", () => {
  assert.equal(apiTarget(undefined), "http://127.0.0.1:3210");
  assert.equal(apiTarget("0.0.0.0:4000"), "http://127.0.0.1:4000");
  assert.equal(apiTarget("[::]:4100"), "http://[::1]:4100");
  assert.throws(() => apiTarget("missing-port"), /invalid ANY2API_BIND/u);
});

test(
  "managed POSIX process groups terminate descendants",
  { skip: process.platform === "win32" },
  async () => {
    const script = [
      "const {spawn}=require('node:child_process');",
      "spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});",
      "setInterval(()=>{},1000);",
    ].join("");
    const child = ManagedProcess.start(process.execPath, ["-e", script], {
      grouped: true,
      stdio: "ignore",
      label: "process tree fixture",
    });
    await new Promise((resolve) => setTimeout(resolve, 100));
    await child.stop({ graceMs: 1_000 });
    const result = await child.wait();
    assert.ok(result.signal || result.code !== null);
  },
);

test(
  "managed POSIX groups still terminate descendants after the leader exits",
  { skip: process.platform === "win32" },
  async () => {
    const script = [
      "const {spawn}=require('node:child_process');",
      "const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:['ignore','ignore','ignore']});",
      "console.log(child.pid);",
      "child.unref();",
    ].join("");
    const child = ManagedProcess.start(process.execPath, ["-e", script], {
      grouped: true,
      stdio: ["ignore", "pipe", "ignore"],
      label: "exited process tree fixture",
    });
    let descendantPid;
    child.child.stdout.setEncoding("utf8");
    for await (const line of child.child.stdout) {
      descendantPid = Number(line.trim());
      break;
    }
    await child.wait();
    await child.stop({ graceMs: 1_000 });
    assert.ok(descendantPid);
    assert.throws(() => process.kill(descendantPid, 0), { code: "ESRCH" });
  },
);
