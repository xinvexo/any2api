import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { RequestLogManagement } from "./RequestLogManagement";

afterEach(() => vi.restoreAllMocks());

test("renders request logs in a table without leaving the page for details", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const path = String(input);
    if (path.startsWith("/api/admin/request-logs?") || path === "/api/admin/request-logs?limit=100") {
      return listResponse([request()]);
    }
    if (path === "/api/admin/request-logs/11111111-1111-4111-8111-111111111111") {
      return detailResponse();
    }
    throw new Error(`unexpected ${path}`);
  });

  renderManagement();

  // Mobile cards + desktop table both mount (CSS hides one); assert both shells exist.
  expect(await screen.findByRole("list", { name: "请求日志列表" })).toBeInTheDocument();
  expect(screen.getByRole("table", { name: "请求日志表格" })).toBeInTheDocument();
  expect(screen.getAllByText("codex-local").length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText("API Key").length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText("44444444…").length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText("成功").length).toBeGreaterThanOrEqual(1);
  expect(screen.getByText(/丢弃/)).toBeInTheDocument();
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
  expect(screen.queryByText("private prompt")).not.toBeInTheDocument();

  const toggle = screen
    .getAllByRole("button")
    .find((button) => button.getAttribute("aria-expanded") === "false");
  expect(toggle).toBeTruthy();
  fireEvent.click(toggle!);

  expect((await screen.findAllByText("Attempt 时间线")).length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText("success").length).toBeGreaterThanOrEqual(1);
  expect(
    screen.getAllByText("11111111-1111-4111-8111-111111111111").length,
  ).toBeGreaterThanOrEqual(1);
  // successful log has no error banner
  expect(screen.queryByText("错误详情")).not.toBeInTheDocument();
  expect(fetchMock.mock.calls.some(([path]) => String(path).includes("/request-logs/11111111"))).toBe(
    true,
  );
});

test("distinguishes an OAuth final upstream source", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    listResponse([
      {
        ...request(),
        credential_id: null,
        oauth_account_id: "55555555-5555-4555-8555-555555555555",
      },
    ]),
  );

  renderManagement();

  expect((await screen.findAllByText("OAuth")).length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText("55555555…").length).toBeGreaterThanOrEqual(1);
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
  await screen.findAllByText("codex-local");
  fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  expect(await screen.findByText(/刷新失败/)).toBeInTheDocument();
  expect(screen.getAllByText("codex-local").length).toBeGreaterThanOrEqual(1);
  expect(fetchMock).toHaveBeenCalledTimes(2);
});

test("paginates request logs from the toolbar", async () => {
  vi.spyOn(globalThis, "fetch").mockResolvedValue(
    listResponse(
      Array.from({ length: 12 }, (_, index) => ({
        ...request(),
        request_id: `11111111-1111-4111-8111-1111111111${String(index).padStart(2, "0")}`,
        public_model: `model-${index + 1}`,
      })),
    ),
  );

  renderManagement();
  expect((await screen.findAllByText("model-1")).length).toBeGreaterThanOrEqual(1);
  // default page size 20 shows all 12
  expect(screen.getAllByText("model-12").length).toBeGreaterThanOrEqual(1);

  fireEvent.change(screen.getAllByLabelText("每页条数")[0]!, { target: { value: "10" } });
  expect(screen.getAllByText("model-1").length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText("model-10").length).toBeGreaterThanOrEqual(1);
  expect(screen.queryByText("model-11")).not.toBeInTheDocument();

  fireEvent.click(screen.getAllByRole("button", { name: "下一页" })[0]!);
  expect(screen.getAllByText("model-11").length).toBeGreaterThanOrEqual(1);
  expect(screen.getAllByText("model-12").length).toBeGreaterThanOrEqual(1);
  expect(screen.queryByText("model-1")).not.toBeInTheDocument();
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

function listResponse(items: unknown[]) {
  return new Response(
    JSON.stringify({
      items,
      telemetry: { queued_records: 0, dropped_records: 2, persisted_records: items.length },
    }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}

function detailResponse() {
  return new Response(
    JSON.stringify({
      request: request(),
      attempts: [
        {
          attempt_no: 1,
          route_target_id: null,
          credential_id: "44444444-4444-4444-8444-444444444444",
          oauth_account_id: null,
          proxy_profile_id: "33333333-3333-4333-8333-333333333333",
          started_at_ms: 1_700_000_000_000,
          duration_ms: 12,
          retry_safety: null,
          error_class: null,
          error_message: null,
          status_code: 200,
          outcome: "success",
        },
      ],
      telemetry: { queued_records: 0, dropped_records: 2, persisted_records: 1 },
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

function request() {
  return {
    request_id: "11111111-1111-4111-8111-111111111111",
    started_at_ms: 1_700_000_000_000,
    config_revision: 3,
    gateway_api_key_id: "22222222-2222-4222-8222-222222222222",
    ingress_protocol: "openai_responses",
    operation: "responses",
    public_model: "codex-local",
    provider_endpoint_id: "33333333-3333-4333-8333-333333333333",
    credential_id: "44444444-4444-4444-8444-444444444444",
    oauth_account_id: null,
    proxy_profile_id: "33333333-3333-4333-8333-333333333333",
    status_code: 200,
    error_class: null,
    error_message: null,
    attempt_count: 1,
    latency_ms: 42,
    first_token_ms: 18,
    input_tokens: 120,
    output_tokens: 45,
    cache_read_tokens: 30,
    cache_write_tokens: 6,
    is_stream: false,
  };
}
