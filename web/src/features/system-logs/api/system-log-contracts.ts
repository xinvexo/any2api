export type SystemLogOutcome = "completed" | "body_error" | "cancelled";

export interface SystemLog {
  requestId: string;
  startedAtMs: number;
  configRevision: number;
  clientIp: string | null;
  method: string;
  path: string;
  httpVersion: string;
  statusCode: number | null;
  durationMs: number;
  responseBytes: number;
  outcome: SystemLogOutcome;
}

export interface SystemLogDetail {
  log: SystemLog;
}

interface SystemLogTelemetry {
  queuedRecords: number;
  inFlightRecords: number;
  droppedRecords: number;
  persistedRecords: number;
}

export interface SystemLogList {
  items: SystemLog[];
  nextCursor: string | null;
  hasMore: boolean;
  telemetry: SystemLogTelemetry;
}

export interface ClearSystemLogsResult {
  deleted: number;
}

export function parseSystemLogList(value: unknown): SystemLogList {
  const record = readRecord(value);
  const items = readArray(record.items).map(parseSystemLog);
  const nextCursor = readCursor(record.next_cursor);
  const hasMore = readBoolean(record.has_more);
  if (
    items.length > 100 ||
    hasMore !== (nextCursor !== null)
  ) {
    throw invalidResponse();
  }
  return {
    items,
    nextCursor,
    hasMore,
    telemetry: parseTelemetry(record.telemetry),
  };
}

export function parseClearSystemLogsResult(value: unknown): ClearSystemLogsResult {
  const record = readRecord(value);
  return { deleted: readNonNegativeInteger(record.deleted) };
}

export function parseSystemLogDetail(value: unknown): SystemLogDetail {
  const record = readRecord(value);
  return { log: parseSystemLog(record.log) };
}

function parseSystemLog(value: unknown): SystemLog {
  const record = readRecord(value);
  return {
    requestId: readString(record.request_id),
    startedAtMs: readNonNegativeInteger(record.started_at_ms),
    configRevision: readPositiveInteger(record.config_revision),
    clientIp: readNullableString(record.client_ip),
    method: readString(record.method),
    path: readString(record.path),
    httpVersion: readHttpVersion(record.http_version),
    statusCode: readNullableStatusCode(record.status_code),
    durationMs: readNonNegativeInteger(record.duration_ms),
    responseBytes: readNonNegativeInteger(record.response_bytes),
    outcome: readOutcome(record.outcome),
  };
}

function parseTelemetry(value: unknown): SystemLogTelemetry {
  const record = readRecord(value);
  return {
    queuedRecords: readNonNegativeInteger(record.queued_records),
    inFlightRecords: readNonNegativeInteger(record.in_flight_records),
    droppedRecords: readNonNegativeInteger(record.dropped_records),
    persistedRecords: readNonNegativeInteger(record.persisted_records),
  };
}

function readHttpVersion(value: unknown): string {
  const version = readString(value);
  if (["HTTP/0.9", "HTTP/1.0", "HTTP/1.1", "HTTP/2", "HTTP/3"].includes(version)) {
    return version;
  }
  throw invalidResponse();
}

function readOutcome(value: unknown): SystemLogOutcome {
  if (value === "completed" || value === "body_error" || value === "cancelled") {
    return value;
  }
  throw invalidResponse();
}

function readRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null) {
    throw invalidResponse();
  }
  return value as Record<string, unknown>;
}

function readArray(value: unknown): unknown[] {
  if (!Array.isArray(value)) {
    throw invalidResponse();
  }
  return value;
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) {
    throw invalidResponse();
  }
  return value;
}

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw invalidResponse();
  }
  return value;
}

function readNullableString(value: unknown): string | null {
  return value === null ? null : readString(value);
}

function readCursor(value: unknown): string | null {
  const cursor = readNullableString(value);
  if (cursor !== null && cursor.length > 1_024) {
    throw invalidResponse();
  }
  return cursor;
}

function readNonNegativeInteger(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw invalidResponse();
  }
  return value;
}

function readPositiveInteger(value: unknown): number {
  const number = readNonNegativeInteger(value);
  if (number === 0) {
    throw invalidResponse();
  }
  return number;
}

function readNullableStatusCode(value: unknown): number | null {
  if (value === null) {
    return null;
  }
  const status = readNonNegativeInteger(value);
  if (status < 100 || status > 999) {
    throw invalidResponse();
  }
  return status;
}

function invalidResponse() {
  return new Error("invalid system log response");
}
