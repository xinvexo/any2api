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
      attempt(1, 404, "The model was not found"),
      attempt(2, 200, null),
      attempt(3, null, "any2api timed out waiting for an upstream response"),
    ]),
  );

  renderDeepLink(`/logs/${requestId}`);

  expect(await screen.findByText("失败 · HTTP 404")).toBeInTheDocument();
  expect(screen.getByText("HTTP 200")).toBeInTheDocument();
  expect(screen.getByText("The model was not found")).toBeInTheDocument();
  expect(screen.getByText("失败 · 未收到上游状态")).toBeInTheDocument();
  expect(screen.getByText("18 ms")).toBeInTheDocument();
  expect(screen.getByText("203.0.113.8")).toBeInTheDocument();
  expect(screen.getByText("120")).toBeInTheDocument();
  expect(screen.getByText("45")).toBeInTheDocument();
  expect(screen.getByText("30")).toBeInTheDocument();
  expect(screen.getByText("frapi · Primary credential")).toBeInTheDocument();
  expect(screen.getByText("Upstream 1 · Credential 1")).toBeInTheDocument();
  expect(screen.getAllByText("DIRECT").length).toBeGreaterThan(1);
  expect(screen.queryByText(/负载均衡/)).not.toBeInTheDocument();
  expect(screen.queryByText(/generic-rustls-hyper-v2/)).not.toBeInTheDocument();
  expect(fetchMock).toHaveBeenCalledTimes(1);
  expect(String(fetchMock.mock.calls[0]?.[0])).toBe(`/api/admin/request-logs/${requestId}`);
});

test("renders an attempt empty state", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    detailResponse([], {
      status_code: 400,
      outcome: "failed",
      error_message: "request validation failed",
      attempt_count: 0,
      first_token_ms: null,
      input_tokens: null,
      output_tokens: null,
      cache_read_tokens: null,
      cache_creation_tokens: null,
    }),
  );

  renderDetail();

  expect(await screen.findByText("没有可展示的尝试")).toBeInTheDocument();
  expect(screen.getByText("request validation failed")).toBeInTheDocument();
  expect(screen.queryByText("返回错误消息")).not.toBeInTheDocument();
});

test("keeps unavailable token telemetry distinct from real zero values", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    detailResponse([], {
      first_token_ms: 0,
      input_tokens: 0,
      output_tokens: null,
      cache_read_tokens: null,
      cache_creation_tokens: null,
    }),
  );

  renderDetail();

  expect(await screen.findByText("0 ms")).toBeInTheDocument();
  expect(screen.getByText("0")).toBeInTheDocument();
  expect(screen.getAllByText("未记录")).toHaveLength(3);
});

test("renders a failed stream separately from its HTTP 200 handshake", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    detailResponse(
      [attempt(1, 200, null, "failed")],
      {
        outcome: "failed",
        error_message: "upstream response stream reported a failure event",
        first_token_ms: null,
      },
    ),
  );

  renderDetail();

  expect(await screen.findByText("失败 200")).toBeInTheDocument();
  expect(screen.getByText("失败 · HTTP 200")).toBeInTheDocument();
  expect(screen.getByText("HTTP 状态").nextElementSibling).toHaveTextContent("200");
  expect(screen.getAllByText("upstream response stream reported a failure event")).toHaveLength(1);
  expect(screen.queryByText("返回错误消息")).not.toBeInTheDocument();
  expect(screen.queryByText("Token 统计")).not.toBeInTheDocument();
  expect(screen.queryByText("首 Token 延迟（TTFT）")).not.toBeInTheDocument();
});

test("renders OpenAI Images protocol and operation labels", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    detailResponse([], {
      ingress_protocol: "openai_images",
      operation: "images_edits",
    }),
  );

  renderDetail();

  expect(await screen.findByText("OpenAI Images")).toBeInTheDocument();
  expect(screen.getByText("/v1/images/edits")).toBeInTheDocument();
});

test("identifies an OAuth account in the attempt that used it", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    detailResponse([
      {
        ...attempt(1, 429, "rate limit reached"),
        provider_endpoint_id: null,
        provider_endpoint_name: null,
        credential_id: null,
        credential_label: null,
        oauth_account_id: "oauth-account-1",
        oauth_account_label: "operator@example.com",
      },
    ]),
  );

  renderDetail();

  expect(await screen.findByText("OAuth · operator@example.com")).toBeInTheDocument();
  expect(screen.getByText("rate limit reached")).toBeInTheDocument();
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

function detailResponse(
  attempts: Record<string, unknown>[],
  requestOverrides: Record<string, unknown> = {},
) {
  return new Response(
    JSON.stringify({
      request: request(requestOverrides),
      attempts,
      telemetry: { queued_records: 0, in_flight_records: 0, dropped_records: 0, persisted_records: 1 },
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

function request(overrides: Record<string, unknown> = {}) {
  return {
    request_id: requestId,
    started_at_ms: 1_700_000_000_000,
    client_ip: "203.0.113.8",
    config_revision: 9,
    gateway_api_key_id: "22222222-2222-4222-8222-222222222222",
    ingress_protocol: "openai_responses",
    operation: "responses",
    public_model: "codex-local",
    thinking_level: null,
    provider_endpoint_id: "33333333-3333-4333-8333-333333333333",
    provider_endpoint_name: "frapi",
    credential_id: "44444444-4444-4444-8444-444444444444",
    credential_label: "Primary credential",
    oauth_account_id: null,
    oauth_account_label: null,
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
    proxy_profile_label: "DIRECT",
    status_code: 200,
    outcome: "success",
    error_message: null,
    attempt_count: 2,
    latency_ms: 30,
    first_token_ms: 18,
    input_tokens: 120,
    output_tokens: 45,
    cache_read_tokens: 30,
    cache_creation_tokens: null,
    is_stream: true,
    ...overrides,
  };
}

function attempt(
  attemptNo: number,
  statusCode: number | null,
  errorMessage: string | null,
  outcome = statusCode !== null && statusCode >= 200 && statusCode < 300 ? "success" : "failed",
): Record<string, unknown> {
  return {
    attempt_no: attemptNo,
    route_target_id: `target-${attemptNo}`,
    provider_endpoint_id: `endpoint-${attemptNo}`,
    provider_endpoint_name: `Upstream ${attemptNo}`,
    credential_id: `credential-${attemptNo}`,
    credential_label: `Credential ${attemptNo}`,
    oauth_account_id: null,
    oauth_account_label: null,
    proxy_profile_id: "00000000-0000-0000-0000-000000000000",
    proxy_profile_label: "DIRECT",
    routing_mode: "balanced",
    failure_scope: outcome === "failed" ? "exact_candidate" : null,
    retry_decision: outcome === "failed" ? "reselect" : null,
    started_at_ms: 1_700_000_000_000 + attemptNo,
    duration_ms: 10,
    error_message: errorMessage,
    status_code: statusCode,
    outcome,
    transport: {
      wire_profile_id: "generic-rustls-hyper-v2",
      wire_profile_version: 2,
      timeout_policy_version: 1,
      resolver_mode: "system",
      proxy_kind: "direct",
      connect_timeout_ms: 10_000,
      read_timeout_ms: 300_000,
      pool_idle_timeout_ms: 50_000,
      routing_generation: 3,
      authentication_version: 5,
      traffic_class: "data_plane",
    },
    stream_timing: {
      first_upstream_frame_ms: 4,
      stream_commit_ms: 5,
      first_downstream_byte_ms: 7,
      stream_cancel_ms: outcome === "cancelled" ? 10 : null,
    },
  };
}
