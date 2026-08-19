import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { AdminRealtimeProvider } from "@/shared/realtime";
import { FakeEventSource } from "@/test/fake-event-source";

import { overviewResourcesQueryKeys } from "./overview-resources-query-keys";
import { useOverviewResources } from "./use-overview-resources";

afterEach(() => {
  FakeEventSource.reset();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("updates the resource query from overview snapshots without a polling interval", async () => {
  vi.stubGlobal("EventSource", FakeEventSource);
  vi.stubGlobal("fetch", vi.fn(async () => jsonResponse(resources(1))));
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

  render(
    <QueryClientProvider client={queryClient}>
      <AdminRealtimeProvider authenticated>
        <ResourceConsumer />
      </AdminRealtimeProvider>
    </QueryClientProvider>,
  );

  expect(await screen.findByText("1")).toBeInTheDocument();
  const options = queryClient.getQueryCache().find({
    queryKey: overviewResourcesQueryKeys.current(),
  })?.options as { refetchInterval?: unknown } | undefined;
  expect(options?.refetchInterval).toBeUndefined();

  act(() => {
    FakeEventSource.instances[0]?.emit(
      "overview_snapshot",
      JSON.stringify({ sampled_at_ms: 2, resources: resources(2), runtime: {} }),
    );
  });

  await waitFor(() => expect(screen.getByText("2")).toBeInTheDocument());
});

function ResourceConsumer() {
  const query = useOverviewResources();
  return <p>{query.data?.sampledAtMs ?? "loading"}</p>;
}

function resources(sampledAt: number) {
  return {
    sampled_at_ms: sampledAt,
    process: { resident_memory_bytes: 20, cpu_usage_percent: 2.4 },
    system: {
      used_memory_bytes: 40,
      total_memory_bytes: 100,
      cpu_usage_percent: 31.7,
    },
    ownership: {
      payload_buffers: {
        heap_current_bytes: 1,
        heap_peak_bytes: 2,
        mapped_current_bytes: 3,
        mapped_peak_bytes: 4,
        http_body_capture_current_bytes: 1,
        http_body_capture_peak_bytes: 2,
      },
    },
  };
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
