import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { AdminRealtimeProvider } from "@/shared/realtime";
import { FakeEventSource } from "@/test/fake-event-source";

import { balancingRuntimeQueryKey, useBalancingRuntime } from "./use-balancing-runtime";

afterEach(() => {
  FakeEventSource.reset();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("updates the balancing query from overview snapshots without a polling interval", async () => {
  vi.stubGlobal("EventSource", FakeEventSource);
  vi.stubGlobal("fetch", vi.fn(async () => jsonResponse(runtime(1))));
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

  render(
    <QueryClientProvider client={queryClient}>
      <AdminRealtimeProvider authenticated>
        <RuntimeConsumer />
      </AdminRealtimeProvider>
    </QueryClientProvider>,
  );

  expect(await screen.findByText("1")).toBeInTheDocument();
  const options = queryClient.getQueryCache().find({
    queryKey: balancingRuntimeQueryKey,
  })?.options as { refetchInterval?: unknown } | undefined;
  expect(options?.refetchInterval).toBeUndefined();

  act(() => {
    FakeEventSource.instances[0]?.emit(
      "overview_snapshot",
      JSON.stringify({ sampled_at_ms: 2, resources: {}, runtime: runtime(2) }),
    );
  });

  await waitFor(() => expect(screen.getByText("2")).toBeInTheDocument());
});

function RuntimeConsumer() {
  const query = useBalancingRuntime();
  return <p>{query.data?.configRevision ?? "loading"}</p>;
}

function runtime(configRevision: number) {
  return {
    config_revision: configRevision,
    scheduler_epoch: 0,
    process: { active_requests: 0, background_tasks: 0, shutdown_phase: "running" },
    transport: null,
    breakers: { closed: 0, open: 0, half_open: 0 },
    telemetry: { queued: 0, in_flight: 0, capacity: 1, dropped: 0 },
    queue: {
      waiting: 0,
      max_waiting: 1,
      timeout_secs: 1,
      on_rate_limited: "wait",
      fallback_on_rate_limit: false,
    },
    totals: {
      credential_count: 0,
      enabled_credential_count: 0,
      limited_credential_count: 0,
      rate_limited_credential_count: 0,
      in_flight: 0,
      requests_in_window: 0,
      fixed_waiters: 0,
      selected: 0,
    },
    providers: [],
  };
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
