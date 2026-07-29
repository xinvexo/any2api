import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { BalancingOverview } from "./BalancingOverview";

afterEach(() => vi.restoreAllMocks());

test("renders fixed-size routing aggregates for a large account collection", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(runtimeResponse()));
  const rendered = renderOverview();

  expect(await screen.findByRole("heading", { name: "请求调度" })).toBeInTheDocument();
  expect(screen.getByText("1,845")).toBeInTheDocument();
  expect(screen.getByText("12 / 800")).toBeInTheDocument();
  expect(screen.getByText(/940 \/ 1,000 个账号已启用/)).toBeInTheDocument();
  expect(screen.getByText("Codex")).toBeInTheDocument();
  expect(screen.getByText("Claude")).toBeInTheDocument();
  expect(screen.getByText("Grok")).toBeInTheDocument();
  expect(screen.queryByText(/调度 Epoch/)).not.toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "调整策略" })).not.toBeInTheDocument();
  expect(screen.queryByText(/配置版本/)).not.toBeInTheDocument();
  expect(screen.queryByText("Credential 健康过滤")).not.toBeInTheDocument();
  expect(screen.queryByText("Endpoint 可用")).not.toBeInTheDocument();
  expect(rendered.container.querySelector(".rounded-\\[14px\\]")).toBeNull();
});

test("renders an empty aggregate without an account directory", async () => {
  const empty = runtimeResponse();
  empty.providers = [];
  empty.totals = {
    credential_count: 0,
    enabled_credential_count: 0,
    limited_credential_count: 0,
    rate_limited_credential_count: 0,
    in_flight: 0,
    requests_in_window: 0,
    fixed_waiters: 0,
    selected: 0,
  };
  vi.spyOn(globalThis, "fetch").mockResolvedValue(jsonResponse(empty));
  renderOverview();

  expect(await screen.findByText("尚未配置可路由账号。")).toBeInTheDocument();
  expect(screen.queryByText("还没有路由 Credential")).not.toBeInTheDocument();
});

function renderOverview() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <MemoryRouter>
      <QueryClientProvider client={client}>
        <BalancingOverview />
      </QueryClientProvider>
    </MemoryRouter>,
  );
}

function runtimeResponse() {
  return {
    config_revision: 3,
    scheduler_epoch: 8,
    queue: { waiting: 1, max_waiting: 128, timeout_secs: 30, on_rate_limited: "wait", fallback_on_rate_limit: false },
    totals: { credential_count: 1_000, enabled_credential_count: 940, limited_credential_count: 800, rate_limited_credential_count: 12, in_flight: 27, requests_in_window: 1_845, fixed_waiters: 2, selected: 42_000 },
    providers: [
      { provider_kind: "codex", credential_count: 600, enabled_credential_count: 560, limited_credential_count: 470, rate_limited_credential_count: 8, in_flight: 18, requests_in_window: 1_200, fixed_waiters: 2, selected: 28_000 },
      { provider_kind: "claude", credential_count: 250, enabled_credential_count: 235, limited_credential_count: 210, rate_limited_credential_count: 3, in_flight: 6, requests_in_window: 400, fixed_waiters: 0, selected: 10_000 },
      { provider_kind: "grok", credential_count: 150, enabled_credential_count: 145, limited_credential_count: 120, rate_limited_credential_count: 1, in_flight: 3, requests_in_window: 245, fixed_waiters: 0, selected: 4_000 },
    ],
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
