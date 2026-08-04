import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { RequestLogManagement } from "./RequestLogManagement";
import { FakeEventSource } from "@/test/fake-event-source";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  FakeEventSource.reset();
});

test("paginates request logs from the toolbar", async () => {
  const items = Array.from({ length: 12 }, (_, index) => ({
    ...request(),
    request_id: `11111111-1111-4111-8111-1111111111${String(index).padStart(2, "0")}`,
    public_model: `model-${index + 1}`,
  }));
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const path = String(input);
    if (path === "/api/admin/request-logs?page_size=20") {
      return listResponse(items, 12, 20, "r1.default");
    }
    if (path === "/api/admin/request-logs?page_size=10") {
      return listResponse(items.slice(0, 10), 12, 10, "r1.page-1", "r1.page-2");
    }
    if (path === "/api/admin/request-logs?page_size=10&cursor=r1.page-2") {
      return listResponse(items.slice(10), 12, 10, "r1.page-2");
    }
    throw new Error(`unexpected ${path}`);
  });

  renderManagement();
  expect((await screen.findAllByText("model-1")).length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText("model-12").length).toBeGreaterThanOrEqual(1);

  fireEvent.click(screen.getAllByRole("combobox", { name: "每页条数" })[0]!);
  fireEvent.click(screen.getByRole("option", { name: "10 条/页" }));
  expect((await screen.findAllByText("model-1")).length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText("model-10").length).toBeGreaterThanOrEqual(1);
  expect(screen.queryByText("model-11")).not.toBeInTheDocument();

  fireEvent.click(screen.getAllByRole("button", { name: "下一页" })[0]!);
  expect((await screen.findAllByText("model-11")).length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText("model-12").length).toBeGreaterThanOrEqual(1);
  expect(screen.queryByText("model-1")).not.toBeInTheDocument();
});

test("returns to the last valid request-log page after the total shrinks", async () => {
  let pinnedFirstPageLoads = 0;
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const path = String(input);
    if (path === "/api/admin/request-logs?page_size=20") {
      return listResponse(
        [{ ...request(), public_model: "model-first" }],
        41,
        20,
        "r1.page-1",
        "r1.page-2",
      );
    }
    if (path === "/api/admin/request-logs?page_size=20&cursor=r1.page-1") {
      pinnedFirstPageLoads += 1;
      return listResponse(
        [{ ...request(), public_model: "model-recovered" }],
        1,
        20,
        "r1.page-1",
      );
    }
    if (path === "/api/admin/request-logs?page_size=20&cursor=r1.page-2") {
      return listResponse(
        [{ ...request(), public_model: "model-second" }],
        41,
        20,
        "r1.page-2",
        "r1.page-3",
      );
    }
    if (path === "/api/admin/request-logs?page_size=20&cursor=r1.page-3") {
      return listResponse([], 1, 20, "r1.page-3");
    }
    throw new Error(`unexpected ${path}`);
  });

  renderManagement();
  expect((await screen.findAllByText("model-first")).length).toBeGreaterThanOrEqual(1);
  fireEvent.click(screen.getAllByRole("button", { name: "下一页" })[0]!);
  expect((await screen.findAllByText("model-second")).length).toBeGreaterThanOrEqual(1);
  fireEvent.click(screen.getAllByRole("button", { name: "下一页" })[0]!);

  expect((await screen.findAllByText("model-recovered")).length).toBeGreaterThanOrEqual(1);
  expect(pinnedFirstPageLoads).toBe(1);
  expect(fetchMock.mock.calls.at(-1)?.[0]).toBe(
    "/api/admin/request-logs?page_size=20&cursor=r1.page-1",
  );
});

test("pins history pages, pauses live updates, and refreshes back to latest", async () => {
  vi.stubGlobal("EventSource", FakeEventSource);
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const path = String(input);
    if (path === "/api/admin/request-logs?page_size=20") {
      return listResponse(
        [{ ...request(), public_model: "model-latest" }],
        21,
        20,
        "r1.page-1",
        "r1.page-2",
      );
    }
    if (path === "/api/admin/request-logs?page_size=20&cursor=r1.page-2") {
      return listResponse(
        [{ ...request(), public_model: "model-history" }],
        21,
        20,
        "r1.page-2",
      );
    }
    if (path === "/api/admin/request-logs?page_size=20&cursor=r1.page-1") {
      return listResponse(
        [{ ...request(), public_model: "model-pinned" }],
        21,
        20,
        "r1.page-1",
        "r1.page-2",
      );
    }
    throw new Error(`unexpected ${path}`);
  });

  renderManagement();
  expect((await screen.findAllByText("model-latest")).length).toBeGreaterThanOrEqual(1);
  const liveSource = FakeEventSource.instances[0];
  expect(liveSource).toBeDefined();

  fireEvent.click(screen.getAllByRole("button", { name: "下一页" })[0]!);
  expect((await screen.findAllByText("model-history")).length).toBeGreaterThanOrEqual(1);
  expect(liveSource?.closed).toBe(true);

  await act(async () => {
    liveSource?.emit("request_logs_changed");
  });
  expect(fetchMock).toHaveBeenCalledTimes(2);

  fireEvent.click(screen.getAllByRole("button", { name: "上一页" })[0]!);
  expect((await screen.findAllByText("model-pinned")).length).toBeGreaterThanOrEqual(1);
  expect(FakeEventSource.instances).toHaveLength(1);

  fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  expect((await screen.findAllByText("model-latest")).length).toBeGreaterThanOrEqual(1);
  expect(fetchMock.mock.calls.at(-1)?.[0]).toBe("/api/admin/request-logs?page_size=20");
  expect(FakeEventSource.instances).toHaveLength(2);
});

function renderManagement() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter>
        <RequestLogManagement />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function listResponse(
  items: unknown[],
  total = items.length,
  pageSize = 20,
  cursor: string | null = items.length > 0 ? "r1.current" : null,
  nextCursor: string | null = null,
) {
  return new Response(
    JSON.stringify({
      items,
      total,
      page_size: pageSize,
      cursor,
      next_cursor: nextCursor,
      telemetry: {
        queued_records: 0,
        in_flight_records: 1,
        dropped_records: 2,
        persisted_records: items.length,
      },
    }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}

function request() {
  return {
    request_id: "11111111-1111-4111-8111-111111111111",
    started_at_ms: 1_700_000_000_000,
    client_ip: "203.0.113.8",
    config_revision: 3,
    gateway_api_key_id: "22222222-2222-4222-8222-222222222222",
    ingress_protocol: "openai_responses",
    operation: "responses",
    public_model: "codex-local",
    thinking_level: "high",
    provider_endpoint_id: "33333333-3333-4333-8333-333333333333",
    provider_endpoint_name: "frapi",
    credential_id: "44444444-4444-4444-8444-444444444444",
    credential_label: "key",
    oauth_account_id: null,
    oauth_account_label: null,
    proxy_profile_id: "33333333-3333-4333-8333-333333333333",
    status_code: 200,
    error_message: null,
    attempt_count: 1,
    latency_ms: 42,
    first_token_ms: 18,
    input_tokens: 120,
    output_tokens: 45,
    cache_read_tokens: 30,
    is_stream: false,
  };
}
