import { expect, test } from "vitest";

import { parseAffinityRuntime } from "./affinity-contracts";

test("parses aggregate-only affinity runtime", () => {
  expect(
    parseAffinityRuntime({
      config_revision: 7,
      soft_binding_count: 2,
      hard_binding_count: 1,
      creating_count: 0,
      credential_counts: [],
      bindings: [],
    }),
  ).toMatchObject({
    configRevision: 7,
    softBindingCount: 2,
    hardBindingCount: 1,
    creatingCount: 0,
  });
});

test("rejects invalid affinity counters", () => {
  expect(() =>
    parseAffinityRuntime({
      config_revision: 1,
      soft_binding_count: -1,
      hard_binding_count: 0,
      creating_count: 0,
      credential_counts: [],
      bindings: [],
    }),
  ).toThrow("invalid affinity response");
});
