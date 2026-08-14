import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { BalancingRuntime } from "../api/balancing-contracts";
import { BalancingSummary } from "./BalancingSummary";

test("shows live transport cache activity instead of static pool capacity", () => {
  const runtime = balancingRuntime();
  const view = render(<BalancingSummary runtime={runtime} />);

  expect(screen.getByText("Transport 缓存命中 / 未命中")).toBeInTheDocument();
  expect(screen.getByText("1,280 / 12")).toBeInTheDocument();
  expect(screen.getByText("本次运行累计 · 当前条目 3 / 64 · 淘汰 2")).toBeInTheDocument();

  view.rerender(
    <BalancingSummary
      runtime={{
        ...runtime,
        transport: { ...runtime.transport!, cacheHits: 1_281 },
      }}
    />,
  );
  expect(screen.getByText("1,281 / 12")).toBeInTheDocument();
});

function balancingRuntime(): BalancingRuntime {
  return {
    configRevision: 1,
    schedulerEpoch: 1,
    process: { activeRequests: 0, backgroundTasks: 0, shutdownPhase: "running" },
    transport: {
      cacheEntries: 3,
      cacheCapacity: 64,
      cacheHits: 1_280,
      cacheMisses: 12,
      cacheEvictions: 2,
    },
    breakers: { closed: 0, open: 0, halfOpen: 0 },
    telemetry: { queued: 0, inFlight: 0, capacity: 4_096, dropped: 0 },
    queue: {
      waiting: 0,
      maxWaiting: 128,
      timeoutSecs: 180,
      onRateLimited: "wait",
      fallbackOnRateLimit: false,
    },
    totals: {
      credentialCount: 0,
      enabledCredentialCount: 0,
      limitedCredentialCount: 0,
      rateLimitedCredentialCount: 0,
      inFlight: 0,
      requestsInWindow: 0,
      fixedWaiters: 0,
      selected: 0,
    },
    providers: [],
  };
}
