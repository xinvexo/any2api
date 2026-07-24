import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { BalancingManagement } from "./BalancingManagement";

afterEach(() => vi.restoreAllMocks());

test("renders live capacity, filtering semantics and scoped health", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(runtimeResponse()));
  renderManagement();

  expect(await screen.findByText("Primary")).toBeInTheDocument();
  expect(screen.getByRole("progressbar", { name: "Primary 当前负载" })).toHaveAttribute("aria-valuenow", "50");
  expect(screen.getByText("生成选中").parentElement).toHaveTextContent("4");
  expect(screen.getByText("生成选中").parentElement).toHaveTextContent("100%");
  expect(screen.getByText("满载过滤").parentElement).toHaveTextContent("2");
  expect(screen.getByText("Endpoint 健康过滤").parentElement).toHaveTextContent("3");
  expect(screen.getByText("Credential 5s")).toBeInTheDocument();
  expect(screen.getByText("Endpoint 可用")).toBeInTheDocument();
  expect(screen.getByText(/不能用于计费或配额/)).toBeInTheDocument();
});

test("renders an empty credential state", async () => {
  const empty = runtimeResponse();
  empty.credentials = [];
  empty.providers = [];
  empty.totals = { credential_count: 0, enabled_credential_count: 0, in_flight: 0, max_concurrency: 0, fixed_waiters: 0, auxiliary_in_flight: 0 };
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(empty));
  renderManagement();

  expect(await screen.findByText("还没有 Provider Credential")).toBeInTheDocument();
});

test("keeps the latest runtime visible when refresh fails", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch")
    .mockResolvedValueOnce(jsonResponse(runtimeResponse()))
    .mockResolvedValueOnce(new Response(JSON.stringify({ error: { code: "runtime", message: "refresh failed" } }), { status: 500, headers: { "Content-Type": "application/json" } }));
  renderManagement();
  await screen.findByText("Primary");
  fireEvent.click(screen.getByRole("button", { name: "刷新" }));

  expect(await screen.findByText(/刷新失败，当前仍显示最近一次有效数据/)).toBeInTheDocument();
  expect(screen.getByText("Primary")).toBeInTheDocument();
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
});

function renderManagement() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}><BalancingManagement /></QueryClientProvider>);
}

function runtimeResponse() {
  return {
    config_revision: 3, scheduler_epoch: 8,
    queue: { waiting: 1, max_waiting: 128, timeout_secs: 30, on_saturated: "wait", fallback_on_saturation: false },
    auxiliary: { in_flight: 1, max_global: 32, max_per_credential: 4 },
    totals: { credential_count: 1, enabled_credential_count: 1, in_flight: 1, max_concurrency: 2, fixed_waiters: 0, auxiliary_in_flight: 1 },
    providers: [{ provider_kind: "codex", credential_count: 1, in_flight: 1, max_concurrency: 2, selected_generation: 4, selected_auxiliary: 1 }],
    credentials: [{ credential_id: "credential-1", credential_source: "provider_credential", label: "Primary", enabled: true, authentication_expired: false, provider_kind: "codex", endpoint_id: "endpoint-1", endpoint_name: "Codex", endpoint_enabled: true, proxy_id: "proxy-1", proxy_name: "DIRECT", proxy_kind: "direct", proxy_enabled: true, in_flight: 1, max_concurrency: 2, fixed_waiters: 0, auxiliary_in_flight: 1, counters: { selected_generation: 4, selected_auxiliary: 1, filtered_capacity: 2, filtered_credential_health: 1, filtered_endpoint_health: 3, filtered_proxy_health: 0 }, models: [{ upstream_model: "gpt-upstream", credential: { status: "cooling", retry_in_ms: 5_000 }, endpoint: { status: "available", retry_in_ms: null }, proxy: { status: "available", retry_in_ms: null } }] }],
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), { status: 200, headers: { "Content-Type": "application/json" } });
}
