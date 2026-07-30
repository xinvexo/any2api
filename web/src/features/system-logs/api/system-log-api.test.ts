import { afterEach, expect, test, vi } from "vitest";

import { getSystemLogs } from "./system-log-api";

afterEach(() => vi.restoreAllMocks());

test("paginates system logs without client-controlled audit headers", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async () => listResponse());

  await getSystemLogs(2, 50);
  await getSystemLogs();

  const firstHeaders = fetchMock.mock.calls[0]?.[1]?.headers as Record<string, string>;
  expect(fetchMock.mock.calls[0]?.[0]).toBe("/api/admin/system-logs?page=2&page_size=50");
  expect(firstHeaders["X-Any2API-Log-Refresh"]).toBeUndefined();
});

function listResponse() {
  return new Response(JSON.stringify({
    items: [],
    total: 0,
    page: 1,
    page_size: 20,
    telemetry: { queued_records: 0, dropped_records: 0, persisted_records: 0 },
  }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
