import { expect, test } from "vitest";

import {
  parseClearSystemLogsResult,
  parseSystemLogDetail,
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
    cursor: "s2.current",
    next_cursor: null,
    telemetry: { queued_records: 1, in_flight_records: 4, dropped_records: 2, persisted_records: 3 },
  });

  expect(value.items.map((item) => item.path)).toEqual([
    "/api/admin/provider-credentials/actual-id",
    "/assets/app%20name.js",
  ]);
  expect(value.items[1]?.statusCode).toBeNull();
  expect(value.telemetry.inFlightRecords).toBe(4);
  expect(value.telemetry.droppedRecords).toBe(2);
  expect(parseClearSystemLogsResult({ deleted: 42 }).deleted).toBe(42);
});

test("parses raw request and response values without redaction", () => {
  const detail = parseSystemLogDetail({
    log: systemLog("/v1/responses"),
    exchange: {
      request: {
        headers: [
          { name: "authorization", value: "Bearer raw-key", encoding: "utf8" },
          { name: "x-binary", value: "/wA=", encoding: "base64" },
        ],
        body: body('{"prompt":"blocked word"}', 25),
      },
      response: {
        headers: [{ name: "set-cookie", value: "session=raw", encoding: "utf8" }],
        body: body("ok", 2),
      },
    },
  });

  expect(detail.log.uri).toBe("/v1/responses?raw=query");
  expect(detail.exchange?.request.headers[0]?.value).toBe("Bearer raw-key");
  expect(detail.exchange?.request.body.content).toContain("blocked word");
});

test("rejects inconsistent exchange and body capture metadata", () => {
  expect(() => parseSystemLogDetail({ log: systemLog("/"), exchange: null })).toThrow(
    "invalid system log response",
  );
  expect(() => parseSystemLogDetail({
    log: systemLog("/"),
    exchange: {
      request: { headers: [], body: { ...body("x", 2), truncated: false } },
      response: { headers: [], body: body("", 0) },
    },
  })).toThrow("invalid system log response");
});

test("rejects unknown outcomes and invalid response counts", () => {
  expect(() =>
    parseSystemLogList({
      items: [{ ...systemLog("/"), outcome: "unknown" }],
      total: 1,
      page: 1,
      page_size: 20,
      cursor: "s2.current",
      next_cursor: null,
      telemetry: { queued_records: 0, in_flight_records: 0, dropped_records: 0, persisted_records: 0 },
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
      cursor: "s2.current",
      next_cursor: null,
      telemetry: { queued_records: 0, in_flight_records: 0, dropped_records: 0, persisted_records: 0 },
    }),
  ).toThrow("invalid system log response");
  expect(() =>
    parseSystemLogList({
      items: [systemLog("/v1/models")],
      total: 1,
      page: 2,
      page_size: 20,
      cursor: "s2.current",
      next_cursor: null,
      telemetry: { queued_records: 0, in_flight_records: 0, dropped_records: 0, persisted_records: 0 },
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
    uri: `${path}?raw=query`,
    http_version: "HTTP/1.1",
    status_code: 200,
    duration_ms: 12,
    response_bytes: 42,
    outcome: "completed",
    exchange_captured: true,
  };
}

function body(content: string, totalBytes: number) {
  return {
    content,
    encoding: "utf8",
    captured_bytes: new TextEncoder().encode(content).length,
    total_bytes: totalBytes,
    complete: true,
    truncated: new TextEncoder().encode(content).length < totalBytes,
  };
}
