import { expect, test } from "vitest";

import { parseAffinityClearResult, parseAffinityRuntime } from "./affinity-contracts";

test("parses redacted affinity runtime contracts", () => {
  expect(
    parseAffinityRuntime({
      config_revision: 7,
      soft_binding_count: 2,
      hard_binding_count: 1,
      creating_count: 0,
      credential_counts: [
        {
          credential_id: "credential-1",
          credential_label: "Primary",
          soft_bindings: 2,
          hard_bindings: 1,
        },
      ],
      bindings: [
        {
          kind: "hard",
          session_hash_prefix: "abcdefghijkl",
          credential_id: "credential-1",
          route_target_id: "target-1",
          upstream_model: "gpt-upstream",
          protocol_dialect: "openai_responses",
          expires_in_ms: 30_000,
        },
      ],
    }),
  ).toMatchObject({
    configRevision: 7,
    softBindingCount: 2,
    hardBindingCount: 1,
    credentialCounts: [{ credentialId: "credential-1", credentialLabel: "Primary" }],
    bindings: [{ sessionHashPrefix: "abcdefghijkl", expiresInMs: 30_000 }],
  });
  expect(parseAffinityClearResult({ cleared_count: 3 })).toEqual({ clearedCount: 3 });
});

test("rejects invalid affinity counters and dialects", () => {
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
  expect(() =>
    parseAffinityRuntime({
      config_revision: 1,
      soft_binding_count: 0,
      hard_binding_count: 1,
      creating_count: 0,
      credential_counts: [],
      bindings: [
        {
          kind: "hard",
          session_hash_prefix: "abcdefghijkl",
          credential_id: "credential-1",
          route_target_id: "target-1",
          upstream_model: "model",
          protocol_dialect: "unknown",
          expires_in_ms: 1,
        },
      ],
    }),
  ).toThrow("invalid affinity response");
});
