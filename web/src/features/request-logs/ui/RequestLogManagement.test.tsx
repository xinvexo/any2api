import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { RequestLogManagement } from "./RequestLogManagement";

afterEach(() => vi.restoreAllMocks());

test("renders recent request metadata without prompt or secret content", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(listResponse([request()]));

  renderManagement();

  const model = await screen.findByText("codex-local");
  expect(model).toHaveClass("break-all");
  expect(screen.getByText("11111111-1111-4111-8111-111111111111")).toBeInTheDocument();
  expect(screen.getByText(/丢弃 2/)).toBeInTheDocument();
  expect(screen.getByRole("link")).toHaveAttribute(
    "href",
    "/logs/11111111-1111-4111-8111-111111111111",
  );
  expect(screen.queryByText("private prompt")).not.toBeInTheDocument();
  expect(screen.queryByText("provider-secret")).not.toBeInTheDocument();
});

test("renders an empty state", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(listResponse([]));

  renderManagement();

  expect(await screen.findByText("还没有请求日志")).toBeInTheDocument();
});

test("renders an initial error and allows retry", async () => {
  const fetchMock = vi
    .spyOn(globalThis, "fetch")
    .mockResolvedValueOnce(errorResponse("request log storage unavailable"))
    .mockResolvedValueOnce(listResponse([]));

  renderManagement();

  expect(await screen.findByText("无法读取请求日志")).toBeInTheDocument();
  expect(screen.getByText("request log storage unavailable")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "重试" }));
  expect(await screen.findByText("还没有请求日志")).toBeInTheDocument();
  expect(fetchMock).toHaveBeenCalledTimes(2);
});

test("keeps the latest data visible when a refresh fails", async () => {
  const fetchMock = vi
    .spyOn(globalThis, "fetch")
    .mockResolvedValueOnce(listResponse([request()]))
    .mockResolvedValueOnce(errorResponse("refresh failed"));

  renderManagement();
  await screen.findByText("codex-local");
  fireEvent.click(screen.getByRole("button", { name: "刷新" }));

  expect(await screen.findByText(/刷新失败，当前仍显示最近一次有效数据/)).toBeInTheDocument();
  expect(screen.getByText("codex-local")).toBeInTheDocument();
  await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
});

function renderManagement() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <MemoryRouter>
      <QueryClientProvider client={client}>
        <RequestLogManagement />
      </QueryClientProvider>
    </MemoryRouter>,
  );
}

function request() {
  return {
    request_id: "11111111-1111-4111-8111-111111111111",
    started_at_ms: 1_700_000_000_000,
    config_revision: 9,
    gateway_api_key_id: "22222222-2222-4222-8222-222222222222",
    ingress_protocol: "openai_responses",
    operation: "responses",
    public_model: "codex-local",
    provider_endpoint_id: "33333333-3333-4333-8333-333333333333",
    credential_id: "44444444-4444-4444-8444-444444444444",
    oauth_account_id: null,
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
    status_code: 200,
    error_class: null,
    attempt_count: 1,
    latency_ms: 30,
    first_token_ms: null,
    input_tokens: null,
    output_tokens: null,
    cache_read_tokens: null,
    cache_write_tokens: null,
    is_stream: false,
  };
}

function listResponse(items: ReturnType<typeof request>[]) {
  return new Response(
    JSON.stringify({
      items,
      telemetry: { queued_records: 0, dropped_records: 2, persisted_records: 8 },
    }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}

function errorResponse(message: string) {
  return new Response(JSON.stringify({ error: { code: "request_log_storage", message } }), {
    status: 500,
    headers: { "Content-Type": "application/json" },
  });
}
