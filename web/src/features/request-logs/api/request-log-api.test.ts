import { afterEach, expect, test, vi } from "vitest";

import { getRequestLogs } from "./request-log-api";

afterEach(() => vi.restoreAllMocks());

test("paginates request logs without client-controlled audit headers", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async () => listResponse());

  await getRequestLogs(2, 50);
  await getRequestLogs();

  const firstHeaders = fetchMock.mock.calls[0]?.[1]?.headers as Record<string, string>;
  expect(fetchMock.mock.calls[0]?.[0]).toBe("/api/admin/request-logs?page=2&page_size=50");
  expect(firstHeaders["X-Any2API-Log-Refresh"]).toBeUndefined();
});

function listResponse() {
  return new Response(
    JSON.stringify({
      items: [],
      total: 0,
      page: 1,
      page_size: 20,
      telemetry: { queued_records: 0, in_flight_records: 0, dropped_records: 0, persisted_records: 0 },
    }),
    { status: 200, headers: { "Content-Type": "application/json" } },
  );
}
