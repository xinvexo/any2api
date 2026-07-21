import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { RequestLogDetail } from "./RequestLogDetail";
import { RequestLogDetailPage } from "@/pages/RequestLogDetailPage";

const requestId = "11111111-1111-4111-8111-111111111111";

afterEach(() => vi.restoreAllMocks());

test("loads a deep-linked request and renders attempts in order", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
    detailResponse([
      attempt(1, "transport_error", null, "definitely_not_sent", "network"),
      attempt(2, "success", 200, null, null),
    ]),
  );

  renderDeepLink(`/logs/${requestId}`);

  expect(await screen.findByText("transport_error")).toBeInTheDocument();
  expect(screen.getByText("success")).toBeInTheDocument();
  expect(fetchMock).toHaveBeenCalledTimes(1);
  expect(String(fetchMock.mock.calls[0]?.[0])).toBe(`/api/admin/request-logs/${requestId}`);
});

test("renders an attempt empty state", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(detailResponse([]));

  renderDetail();

  expect(await screen.findByText("没有可展示的 Attempt")).toBeInTheDocument();
});

test("renders a terminal not-found state without a retry action", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    errorResponse(404, "request_log_not_found", "request log not found"),
  );

  renderDetail();

  expect(await screen.findByText("这条请求日志不存在")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "返回请求日志" })).toHaveAttribute("href", "/logs");
  expect(screen.queryByRole("button", { name: "重试" })).not.toBeInTheDocument();
});

test("renders a retryable error and a route back to the list", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    errorResponse(500, "request_log_storage", "request log storage unavailable"),
  );

  renderDetail();

  expect(await screen.findByText("无法读取这条请求")).toBeInTheDocument();
  expect(screen.getByText("request log storage unavailable")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "返回请求日志" })).toHaveAttribute("href", "/logs");
});

function renderDetail() {
  return renderWithQuery(
    <MemoryRouter>
      <RequestLogDetail requestId={requestId} />
    </MemoryRouter>,
  );
}

function renderDeepLink(path: string) {
  return renderWithQuery(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/logs/:requestId" element={<RequestLogDetailPage />} />
      </Routes>
    </MemoryRouter>,
  );
}

function renderWithQuery(children: React.ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}>{children}</QueryClientProvider>);
}

function detailResponse(attempts: ReturnType<typeof attempt>[]) {
  return new Response(
    JSON.stringify({
      request: request(),
      attempts,
      telemetry: { queued_records: 0, dropped_records: 0, persisted_records: 1 },
    }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}

function errorResponse(status: number, code: string, message: string) {
  return new Response(JSON.stringify({ error: { code, message } }), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function request() {
  return {
    request_id: requestId,
    started_at_ms: 1_700_000_000_000,
    config_revision: 9,
    gateway_api_key_id: "22222222-2222-4222-8222-222222222222",
    ingress_protocol: "openai_responses",
    operation: "responses",
    public_model: "codex-local",
    provider_endpoint_id: "33333333-3333-4333-8333-333333333333",
    credential_id: "44444444-4444-4444-8444-444444444444",
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
    status_code: 200,
    error_class: null,
    attempt_count: 2,
    latency_ms: 30,
    first_token_ms: null,
    input_tokens: null,
    output_tokens: null,
    cache_read_tokens: null,
    cache_write_tokens: null,
    is_stream: false,
  };
}

function attempt(
  attemptNo: number,
  outcome: string,
  statusCode: number | null,
  retrySafety: string | null,
  errorClass: string | null,
) {
  return {
    attempt_no: attemptNo,
    route_target_id: `target-${attemptNo}`,
    credential_id: `credential-${attemptNo}`,
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
    started_at_ms: 1_700_000_000_000 + attemptNo,
    duration_ms: 10,
    retry_safety: retrySafety,
    error_class: errorClass,
    status_code: statusCode,
    outcome,
  };
}
