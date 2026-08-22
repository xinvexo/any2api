import { expect, test } from "vitest";

import { parseBalancingRuntime } from "./balancing-contracts";

test("parses aggregate-only balancing runtime", () => {
  const parsed = parseBalancingRuntime(runtimeResponse());

  expect(parsed.process).toEqual({ activeRequests: 4, backgroundTasks: 6, shutdownPhase: "running" });
  expect(parsed.transport).toEqual({
    cacheEntries: 3,
    cacheCapacity: 64,
    cacheHits: 1_280,
    cacheMisses: 12,
    cacheEvictions: 2,
  });
  expect(parsed.breakers).toEqual({ closed: 8, open: 1, halfOpen: 2 });
  expect(parsed.telemetry).toEqual({ queued: 4, inFlight: 1, capacity: 100_000, dropped: 7 });
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
    credentialCount: 600,
    selected: 28_000,
  });
  expect(parsed.providers[2]?.providerKind).toBe("grok");
  expect(parsed.providers[3]?.providerKind).toBe("kimi");
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
    process: { active_requests: 4, background_tasks: 6, shutdown_phase: "running" },
    transport: {
      cache_entries: 3,
      cache_capacity: 64,
      cache_hits: 1_280,
      cache_misses: 12,
      cache_evictions: 2,
    },
    breakers: { closed: 8, open: 1, half_open: 2 },
    telemetry: { queued: 4, in_flight: 1, capacity: 100_000, dropped: 7 },
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
      provider("codex", 600, 560, 470, 8, 18, 1_200, 2, 28_000),
      provider("claude", 250, 235, 210, 3, 6, 400, 0, 10_000),
      provider("grok", 100, 95, 80, 1, 3, 200, 0, 3_000),
      provider("kimi", 50, 50, 40, 0, 0, 45, 0, 1_000),
    ],
  };
}

function provider(
  provider_kind: string,
  credential_count: number,
  enabled_credential_count: number,
  limited_credential_count: number,
  rate_limited_credential_count: number,
  in_flight: number,
  requests_in_window: number,
  fixed_waiters: number,
  selected: number,
) {
  return {
    provider_kind,
    credential_count,
    enabled_credential_count,
    limited_credential_count,
    rate_limited_credential_count,
    in_flight,
    requests_in_window,
    fixed_waiters,
    selected,
  };
}
