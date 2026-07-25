import { expect, test } from "vitest";

import { parseBalancingRuntime } from "./balancing-contracts";

test("parses aggregate-only balancing runtime", () => {
  const parsed = parseBalancingRuntime(runtimeResponse());

  expect(parsed.queue).toEqual({
    waiting: 1,
    maxWaiting: 128,
    timeoutSecs: 30,
    onRateLimited: "wait",
    fallbackOnRateLimit: false,
  });
  expect(parsed.totals).toEqual({
    credentialCount: 1_000,
    enabledCredentialCount: 940,
    limitedCredentialCount: 800,
    rateLimitedCredentialCount: 12,
    inFlight: 27,
    requestsInWindow: 1_845,
    fixedWaiters: 2,
    selected: 42_000,
  });
  expect(parsed.providers[0]).toMatchObject({
    providerKind: "codex",
    credentialCount: 700,
    selected: 30_000,
  });
  expect("credentials" in parsed).toBe(false);
});

test("rejects invalid aggregate counters", () => {
  const value = runtimeResponse();
  value.totals.selected = -1;
  expect(() => parseBalancingRuntime(value)).toThrow("invalid balancing runtime response");
});

function runtimeResponse() {
  return {
    config_revision: 3,
    scheduler_epoch: 8,
    queue: {
      waiting: 1,
      max_waiting: 128,
      timeout_secs: 30,
      on_rate_limited: "wait",
      fallback_on_rate_limit: false,
    },
    totals: {
      credential_count: 1_000,
      enabled_credential_count: 940,
      limited_credential_count: 800,
      rate_limited_credential_count: 12,
      in_flight: 27,
      requests_in_window: 1_845,
      fixed_waiters: 2,
      selected: 42_000,
    },
    providers: [
      {
        provider_kind: "codex",
        credential_count: 700,
        enabled_credential_count: 660,
        limited_credential_count: 560,
        rate_limited_credential_count: 8,
        in_flight: 20,
        requests_in_window: 1_400,
        fixed_waiters: 2,
        selected: 30_000,
      },
      {
        provider_kind: "claude",
        credential_count: 300,
        enabled_credential_count: 280,
        limited_credential_count: 240,
        rate_limited_credential_count: 4,
        in_flight: 7,
        requests_in_window: 445,
        fixed_waiters: 0,
        selected: 12_000,
      },
    ],
  };
}
