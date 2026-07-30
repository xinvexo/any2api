import { expect, test } from "vitest";

import {
  parseClearSystemLogsResult,
  parseSystemLogList,
} from "./system-log-contracts";

test("parses exact HTTP paths and nullable pre-response status", () => {
  const value = parseSystemLogList({
    items: [
      systemLog("/api/admin/provider-credentials/actual-id"),
      { ...systemLog("/assets/app%20name.js"), status_code: null, outcome: "cancelled" },
    ],
    total: 2,
    page: 1,
    page_size: 20,
    telemetry: { queued_records: 1, dropped_records: 2, persisted_records: 3 },
  });

  expect(value.items.map((item) => item.path)).toEqual([
    "/api/admin/provider-credentials/actual-id",
    "/assets/app%20name.js",
  ]);
  expect(value.items[1]?.statusCode).toBeNull();
  expect(value.telemetry.droppedRecords).toBe(2);
  expect(parseClearSystemLogsResult({ deleted: 42 }).deleted).toBe(42);
});

test("rejects unknown outcomes and invalid response counts", () => {
  expect(() =>
    parseSystemLogList({
      items: [{ ...systemLog("/"), outcome: "unknown" }],
      total: 1,
      page: 1,
      page_size: 20,
      telemetry: { queued_records: 0, dropped_records: 0, persisted_records: 0 },
    }),
  ).toThrow("invalid system log response");
  expect(() => parseClearSystemLogsResult({ deleted: -1 })).toThrow(
    "invalid system log response",
  );
  expect(() =>
    parseSystemLogList({
      items: [systemLog("/v1/models")],
      total: 0,
      page: 1,
      page_size: 20,
      telemetry: { queued_records: 0, dropped_records: 0, persisted_records: 0 },
    }),
  ).toThrow("invalid system log response");
});

function systemLog(path: string) {
  return {
    request_id: "11111111-1111-4111-8111-111111111111",
    started_at_ms: 1_700_000_000_000,
    config_revision: 3,
    client_ip: "203.0.113.8",
    method: "GET",
    path,
    http_version: "HTTP/1.1",
    status_code: 200,
    duration_ms: 12,
    response_bytes: 42,
    outcome: "completed",
  };
}
