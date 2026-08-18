import assert from "node:assert/strict";
import test from "node:test";

import { BackendBuildCoordinator } from "../dev/backend-coordinator.mjs";

test("rapid source changes coalesce into one build and deployment", async () => {
  let builds = 0;
  let deployments = 0;
  const coordinator = new BackendBuildCoordinator({
    debounceMs: 10,
    build: async () => `artifact-${++builds}`,
    deploy: async () => {
      deployments += 1;
    },
  });
  for (let index = 0; index < 100; index += 1) coordinator.request();
  await delay(30);
  await coordinator.waitForIdle();
  assert.equal(builds, 1);
  assert.equal(deployments, 1);
});

test("a change during build discards the stale result and never overlaps builds", async () => {
  let builds = 0;
  let active = 0;
  let maximumActive = 0;
  let releaseFirst;
  const firstBuild = new Promise((resolve) => (releaseFirst = resolve));
  const deployed = [];
  const coordinator = new BackendBuildCoordinator({
    build: async ({ epoch }) => {
      builds += 1;
      active += 1;
      maximumActive = Math.max(maximumActive, active);
      if (builds === 1) await firstBuild;
      active -= 1;
      return `artifact-${epoch}`;
    },
    deploy: async (artifact) => deployed.push(artifact),
  });
  coordinator.request({ immediate: true });
  await waitFor(() => builds === 1);
  coordinator.request({ immediate: true });
  releaseFirst();
  await waitFor(() => builds === 2);
  await coordinator.waitForIdle();
  assert.equal(maximumActive, 1);
  assert.deepEqual(deployed, ["artifact-2"]);
});

test("a change during deployment never launches a stale artifact", async () => {
  let builds = 0;
  let releaseDeployment;
  const deploymentPaused = new Promise((resolve) => (releaseDeployment = resolve));
  const deployed = [];
  const coordinator = new BackendBuildCoordinator({
    build: async ({ epoch }) => `artifact-${++builds}-${epoch}`,
    deploy: async (artifact, epoch, isCurrent) => {
      if (epoch === 1) await deploymentPaused;
      if (isCurrent()) deployed.push(artifact);
    },
  });
  coordinator.request({ immediate: true });
  await waitFor(() => builds === 1);
  coordinator.request({ immediate: true });
  releaseDeployment();
  await waitFor(() => builds === 2);
  await coordinator.waitForIdle();
  assert.deepEqual(deployed, ["artifact-2-2"]);
});

test("a failed build leaves deployment untouched and a later change recovers", async () => {
  let shouldFail = true;
  let failures = 0;
  const deployed = [];
  const coordinator = new BackendBuildCoordinator({
    build: async ({ epoch }) => {
      if (shouldFail) throw new Error("compile failed");
      return `artifact-${epoch}`;
    },
    deploy: async (artifact) => deployed.push(artifact),
    onBuildFailure: () => {
      failures += 1;
    },
  });
  coordinator.request({ immediate: true });
  await coordinator.waitForIdle();
  assert.equal(failures, 1);
  assert.deepEqual(deployed, []);
  shouldFail = false;
  coordinator.request({ immediate: true });
  await coordinator.waitForIdle();
  assert.deepEqual(deployed, ["artifact-2"]);
});

async function waitFor(predicate) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (predicate()) return;
    await delay(5);
  }
  throw new Error("timed out waiting for test state");
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
