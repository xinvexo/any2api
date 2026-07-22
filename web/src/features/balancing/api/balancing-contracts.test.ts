import { expect, test } from "vitest";

import { parseBalancingRuntime } from "./balancing-contracts";

test("parses capacity counters and scoped health", () => {
  const parsed = parseBalancingRuntime(runtimeResponse());

  expect(parsed.queue).toEqual({
    waiting: 1,
    maxWaiting: 128,
    timeoutSecs: 30,
    onSaturated: "wait",
    fallbackOnSaturation: false,
  });
  expect(parsed.credentials[0]?.counters.filteredEndpointHealth).toBe(3);
  expect(parsed.credentials[0]?.models[0]?.credential).toEqual({
    status: "cooling",
    retryInMs: 5_000,
  });
});

test("rejects inconsistent cooling state", () => {
  const value = runtimeResponse();
  value.credentials[0].models[0].credential.retry_in_ms = null;
  expect(() => parseBalancingRuntime(value)).toThrow("invalid balancing runtime response");
});

function runtimeResponse() {
  return {
    config_revision: 3,
    scheduler_epoch: 8,
    queue: { waiting: 1, max_waiting: 128, timeout_secs: 30, on_saturated: "wait", fallback_on_saturation: false },
    auxiliary: { in_flight: 1, max_global: 32, max_per_credential: 4 },
    totals: { credential_count: 1, enabled_credential_count: 1, in_flight: 1, max_concurrency: 2, fixed_waiters: 0, auxiliary_in_flight: 1 },
    providers: [{ provider_kind: "codex", credential_count: 1, in_flight: 1, max_concurrency: 2, selected_generation: 4, selected_auxiliary: 1 }],
    credentials: [{
      credential_id: "credential-1", label: "Primary", enabled: true,
      provider_kind: "codex", endpoint_id: "endpoint-1", endpoint_name: "Codex",
      endpoint_enabled: true, proxy_id: "proxy-1", proxy_name: "DIRECT", proxy_kind: "direct", proxy_enabled: true,
      in_flight: 1, max_concurrency: 2, fixed_waiters: 0, auxiliary_in_flight: 1,
      counters: { selected_generation: 4, selected_auxiliary: 1, filtered_capacity: 2, filtered_credential_health: 1, filtered_endpoint_health: 3, filtered_proxy_health: 0 },
      models: [{ upstream_model: "gpt-upstream", credential: { status: "cooling", retry_in_ms: 5_000 as number | null }, endpoint: { status: "available", retry_in_ms: null }, proxy: { status: "available", retry_in_ms: null } }],
    }],
  };
}
