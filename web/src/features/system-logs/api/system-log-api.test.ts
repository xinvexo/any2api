import { afterEach, expect, test, vi } from "vitest";

import { getSystemLogs } from "./system-log-api";

afterEach(() => vi.restoreAllMocks());

test("marks only automatic system log reads", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async () => listResponse());

  await getSystemLogs(2, 50, undefined, "automatic");
  await getSystemLogs();

  const automaticHeaders = fetchMock.mock.calls[0]?.[1]?.headers as Record<string, string>;
  const ordinaryHeaders = fetchMock.mock.calls[1]?.[1]?.headers as Record<string, string>;
  expect(fetchMock.mock.calls[0]?.[0]).toBe("/api/admin/system-logs?page=2&page_size=50");
  expect(automaticHeaders["X-Any2API-Log-Refresh"]).toBe("automatic");
  expect(ordinaryHeaders["X-Any2API-Log-Refresh"]).toBeUndefined();
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
