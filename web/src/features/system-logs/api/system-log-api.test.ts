import { afterEach, expect, test, vi } from "vitest";

import { getSystemLog, getSystemLogs } from "./system-log-api";

afterEach(() => vi.restoreAllMocks());

test("paginates system logs without client-controlled audit headers", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async () => listResponse());

  await getSystemLogs(2, 50);
  await getSystemLogs();

  const firstHeaders = fetchMock.mock.calls[0]?.[1]?.headers as Record<string, string>;
  expect(fetchMock.mock.calls[0]?.[0]).toBe("/api/admin/system-logs?page=2&page_size=50");
  expect(firstHeaders["X-Any2API-Log-Refresh"]).toBeUndefined();
});

test("loads one raw system log exchange by request ID", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockResolvedValue(
    new Response(JSON.stringify(detailResponse()), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    }),
  );

  await getSystemLog("11111111-1111-4111-8111-111111111111");

  expect(fetchMock.mock.calls[0]?.[0]).toBe(
    "/api/admin/system-logs/11111111-1111-4111-8111-111111111111",
  );
});

function listResponse() {
  return new Response(JSON.stringify({
    items: [],
    total: 0,
    page: 1,
    page_size: 20,
    telemetry: { queued_records: 0, in_flight_records: 0, dropped_records: 0, persisted_records: 0 },
  }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function detailResponse() {
  return {
    log: {
      request_id: "11111111-1111-4111-8111-111111111111",
      started_at_ms: 1_700_000_000_000,
      config_revision: 3,
      client_ip: "203.0.113.8",
      method: "POST",
      path: "/v1/responses",
      uri: "/v1/responses?raw=true",
      http_version: "HTTP/1.1",
      status_code: 200,
      duration_ms: 12,
      response_bytes: 2,
      outcome: "completed",
      exchange_captured: true,
    },
    exchange: {
      request: {
        headers: [],
        body: {
          content: "{}",
          encoding: "utf8",
          captured_bytes: 2,
          total_bytes: 2,
          complete: true,
          truncated: false,
        },
      },
      response: {
        headers: [],
        body: {
          content: "ok",
          encoding: "utf8",
          captured_bytes: 2,
          total_bytes: 2,
          complete: true,
          truncated: false,
        },
      },
    },
  };
}
