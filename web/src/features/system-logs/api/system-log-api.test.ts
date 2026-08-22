import { afterEach, expect, test, vi } from "vitest";

import { getSystemLog, getSystemLogs } from "./system-log-api";

afterEach(() => vi.restoreAllMocks());

test("serializes system log cursors without client-controlled audit headers", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async () => listResponse());

  await getSystemLogs(false, "s5.0.cursor");
  await getSystemLogs();

  const firstHeaders = fetchMock.mock.calls[0]?.[1]?.headers as Record<string, string>;
  expect(fetchMock.mock.calls[0]?.[0]).toBe(
    "/api/admin/system-logs?show_admin_operations=false&cursor=s5.0.cursor",
  );
  expect(fetchMock.mock.calls[1]?.[0]).toBe(
    "/api/admin/system-logs?show_admin_operations=true",
  );
  expect(firstHeaders["X-Any2API-Log-Refresh"]).toBeUndefined();
});

test("loads one system log metadata record by request ID", async () => {
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
    next_cursor: null,
    has_more: false,
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
      http_version: "HTTP/1.1",
      status_code: 200,
      duration_ms: 12,
      response_bytes: 2,
      outcome: "completed",
    },
  };
}
