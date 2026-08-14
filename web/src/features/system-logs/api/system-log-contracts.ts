export type SystemLogOutcome = "completed" | "body_error" | "cancelled";
export type SystemLogByteEncoding = "utf8" | "base64";

export interface SystemLog {
  requestId: string;
  startedAtMs: number;
  configRevision: number;
  clientIp: string | null;
  method: string;
  path: string;
  uri: string;
  httpVersion: string;
  statusCode: number | null;
  durationMs: number;
  responseBytes: number;
  outcome: SystemLogOutcome;
  exchangeCaptured: boolean;
}

export interface SystemLogHeader {
  name: string;
  value: string;
  encoding: SystemLogByteEncoding;
}

export interface SystemLogBody {
  content: string;
  encoding: SystemLogByteEncoding;
  capturedBytes: number;
  totalBytes: number;
  complete: boolean;
  truncated: boolean;
}

export interface SystemLogMessage {
  headers: SystemLogHeader[];
  body: SystemLogBody;
}

export interface SystemLogExchange {
  request: SystemLogMessage;
  response: SystemLogMessage;
}

export interface SystemLogDetail {
  log: SystemLog;
  exchange: SystemLogExchange | null;
}

interface SystemLogTelemetry {
  queuedRecords: number;
  inFlightRecords: number;
  droppedRecords: number;
  persistedRecords: number;
}

export interface SystemLogList {
  items: SystemLog[];
  total: number;
  page: number;
  pageSize: number;
  cursor: string | null;
  nextCursor: string | null;
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
  const cursor = readCursor(record.cursor);
  const nextCursor = readCursor(record.next_cursor);
  if (
    pageSize > 100 ||
    page > Math.max(1, Math.ceil(total / pageSize)) ||
    items.length > pageSize ||
    items.length > total ||
    (items.length > 0 && cursor === null) ||
    (nextCursor !== null && (items.length === 0 || cursor === null || nextCursor === cursor))
  ) {
    throw invalidResponse();
  }
  return {
    items,
    total,
    page,
    pageSize,
    cursor,
    nextCursor,
    telemetry: parseTelemetry(record.telemetry),
  };
}

export function parseClearSystemLogsResult(value: unknown): ClearSystemLogsResult {
  const record = readRecord(value);
  return { deleted: readNonNegativeInteger(record.deleted) };
}

export function parseSystemLogDetail(value: unknown): SystemLogDetail {
  const record = readRecord(value);
  const log = parseSystemLog(record.log);
  const exchange = record.exchange === null ? null : parseExchange(record.exchange);
  if (log.exchangeCaptured !== (exchange !== null)) {
    throw invalidResponse();
  }
  return { log, exchange };
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
    uri: readString(record.uri),
    httpVersion: readHttpVersion(record.http_version),
    statusCode: readNullableStatusCode(record.status_code),
    durationMs: readNonNegativeInteger(record.duration_ms),
    responseBytes: readNonNegativeInteger(record.response_bytes),
    outcome: readOutcome(record.outcome),
    exchangeCaptured: readBoolean(record.exchange_captured),
  };
}

function parseExchange(value: unknown): SystemLogExchange {
  const record = readRecord(value);
  return {
    request: parseMessage(record.request),
    response: parseMessage(record.response),
  };
}

function parseMessage(value: unknown): SystemLogMessage {
  const record = readRecord(value);
  return {
    headers: readArray(record.headers).map(parseHeader),
    body: parseBody(record.body),
  };
}

function parseHeader(value: unknown): SystemLogHeader {
  const record = readRecord(value);
  return {
    name: readString(record.name),
    value: readText(record.value),
    encoding: readEncoding(record.encoding),
  };
}

function parseBody(value: unknown): SystemLogBody {
  const record = readRecord(value);
  const capturedBytes = readNonNegativeInteger(record.captured_bytes);
  const totalBytes = readNonNegativeInteger(record.total_bytes);
  const truncated = readBoolean(record.truncated);
  if (capturedBytes > totalBytes || truncated !== (capturedBytes < totalBytes)) {
    throw invalidResponse();
  }
  return {
    content: readText(record.content),
    encoding: readEncoding(record.encoding),
    capturedBytes,
    totalBytes,
    complete: readBoolean(record.complete),
    truncated,
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

function readText(value: unknown): string {
  if (typeof value !== "string") {
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

function readEncoding(value: unknown): SystemLogByteEncoding {
  if (value === "utf8" || value === "base64") {
    return value;
  }
  throw invalidResponse();
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
