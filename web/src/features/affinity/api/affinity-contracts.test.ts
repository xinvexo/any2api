import { expect, test } from "vitest";

import { parseAffinityRuntime } from "./affinity-contracts";

test("parses aggregate-only affinity runtime", () => {
  expect(
    parseAffinityRuntime({
      config_revision: 7,
      affinity_enabled: true,
      active_session_count: 3,
      creating_session_count: 0,
    }),
  ).toMatchObject({
    configRevision: 7,
    affinityEnabled: true,
    activeSessionCount: 3,
    creatingSessionCount: 0,
  });
});

test("rejects invalid affinity counters", () => {
  expect(() =>
    parseAffinityRuntime({
      config_revision: 1,
      affinity_enabled: false,
      active_session_count: -1,
      creating_session_count: 0,
    }),
  ).toThrow("invalid affinity response");
});
