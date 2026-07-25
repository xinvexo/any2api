import { expect, test } from "vitest";

import { parseBalancingRuntime } from "./balancing-contracts";

test("parses RPM counters and scoped health", () => {
  const parsed = parseBalancingRuntime(runtimeResponse());

  expect(parsed.queue).toEqual({
    waiting: 1,
    maxWaiting: 128,
    timeoutSecs: 30,
    onRateLimited: "wait",
    fallbackOnRateLimit: false,
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

test("rejects an RPM limit above the supported maximum", () => {
  const value = runtimeResponse();
  value.credentials[0].requests_per_minute = 100_001;
  expect(() => parseBalancingRuntime(value)).toThrow("invalid balancing runtime response");
});

test("parses OAuth runtime credentials without exposing a Provider Endpoint", () => {
  const value = runtimeResponse();
  value.credentials[0].credential_source = "oauth_account";
  value.credentials[0].endpoint_id = null as unknown as string;
  value.credentials[0].endpoint_name = null as unknown as string;

  const parsed = parseBalancingRuntime(value);

  expect(parsed.credentials[0]?.credentialSource).toBe("oauth_account");
  expect(parsed.credentials[0]?.endpointId).toBeNull();
  expect(parsed.credentials[0]?.endpointName).toBeNull();
});

function runtimeResponse() {
  return {
    config_revision: 3,
    scheduler_epoch: 8,
    queue: { waiting: 1, max_waiting: 128, timeout_secs: 30, on_rate_limited: "wait", fallback_on_rate_limit: false },
    totals: { credential_count: 1, enabled_credential_count: 1, limited_credential_count: 1, rate_limited_credential_count: 0, in_flight: 1, requests_in_window: 1, fixed_waiters: 0 },
    providers: [{ provider_kind: "codex", credential_count: 1, limited_credential_count: 1, rate_limited_credential_count: 0, in_flight: 1, requests_in_window: 1, selected: 4 }],
    credentials: [{
      credential_id: "credential-1", credential_source: "provider_credential", label: "Primary", enabled: true, authentication_expired: false,
      provider_kind: "codex", endpoint_id: "endpoint-1", endpoint_name: "Codex",
      endpoint_enabled: true, proxy_id: "proxy-1", proxy_name: "DIRECT", proxy_kind: "direct", proxy_enabled: true,
      in_flight: 1, requests_per_minute: 2, requests_in_window: 1, remaining_requests: 1, retry_in_ms: null, fixed_waiters: 0,
      counters: { selected: 4, filtered_rate_limit: 2, filtered_credential_health: 1, filtered_endpoint_health: 3, filtered_proxy_health: 0 },
      models: [{ upstream_model: "gpt-upstream", credential: { status: "cooling", retry_in_ms: 5_000 as number | null }, endpoint: { status: "available", retry_in_ms: null }, proxy: { status: "available", retry_in_ms: null } }],
    }],
  };
}
