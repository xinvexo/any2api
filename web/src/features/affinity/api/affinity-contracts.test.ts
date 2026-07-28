import { expect, test } from "vitest";

import { parseAffinityRuntime } from "./affinity-contracts";

test("parses aggregate-only affinity runtime", () => {
  expect(
    parseAffinityRuntime({
      config_revision: 7,
      binding_count: 3,
      creating_count: 0,
      credential_counts: [],
      bindings: [],
    }),
  ).toMatchObject({
    configRevision: 7,
    bindingCount: 3,
    creatingCount: 0,
  });
});

test("rejects invalid affinity counters", () => {
  expect(() =>
    parseAffinityRuntime({
      config_revision: 1,
      binding_count: -1,
      creating_count: 0,
      credential_counts: [],
      bindings: [],
    }),
  ).toThrow("invalid affinity response");
});
