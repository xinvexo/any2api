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

interface SystemLogTelemetry {
  queuedRecords: number;
  droppedRecords: number;
  persistedRecords: number;
}

export interface SystemLogList {
  items: SystemLog[];
  total: number;
  page: number;
  pageSize: number;
  telemetry: SystemLogTelemetry;
}

export interface ClearSystemLogsResult {
  deleted: number;
}

export function parseSystemLogList(value: unknown): SystemLogList {
  const record = readRecord(value);
  const items = readArray(record.items).map(parseSystemLog);
  const total = readNonNegativeInteger(record.total);
  const page = readPositiveInteger(record.page);
  const pageSize = readPositiveInteger(record.page_size);
  if (pageSize > 100 || items.length > pageSize || items.length > total) {
    throw invalidResponse();
  }
  return {
    items,
    total,
    page,
    pageSize,
    telemetry: parseTelemetry(record.telemetry),
  };
}

export function parseClearSystemLogsResult(value: unknown): ClearSystemLogsResult {
  const record = readRecord(value);
  return { deleted: readNonNegativeInteger(record.deleted) };
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

function readNullableString(value: unknown): string | null {
  return value === null ? null : readString(value);
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
