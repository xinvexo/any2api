export type RequestLogProtocol = "openai_responses" | "codex_backend" | "anthropic_messages";
export type RequestLogOperation =
  | "responses"
  | "responses_compact"
  | "messages"
  | "messages_count_tokens";
export type RequestLogErrorClass =
  | "invalid_request"
  | "authentication"
  | "permission_denied"
  | "quota_exhausted"
  | "rate_limited"
  | "model_unavailable"
  | "operation_unavailable"
  | "proxy"
  | "network"
  | "upstream"
  | "cancelled"
  | "internal";
export type RequestAttemptOutcome =
  | "success"
  | "transport_error"
  | "upstream_error"
  | "invalid_response"
  | "local_error"
  | "stream_error"
  | "cancelled";
export type RequestRetrySafety =
  | "definitely_not_sent"
  | "rejected_before_execution"
  | "idempotent"
  | "ambiguous";

export interface RequestLog {
  requestId: string;
  startedAtMs: number;
  configRevision: number;
  gatewayApiKeyId: string | null;
  ingressProtocol: RequestLogProtocol;
  operation: RequestLogOperation;
  publicModel: string | null;
  providerEndpointId: string | null;
  credentialId: string | null;
  proxyProfileId: string | null;
  statusCode: number;
  errorClass: RequestLogErrorClass | null;
  attemptCount: number;
  latencyMs: number;
  firstTokenMs: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheWriteTokens: number | null;
  isStream: boolean;
}

export interface RequestAttempt {
  attemptNo: number;
  routeTargetId: string | null;
  credentialId: string | null;
  proxyProfileId: string | null;
  startedAtMs: number;
  durationMs: number;
  retrySafety: RequestRetrySafety | null;
  errorClass: RequestLogErrorClass | null;
  statusCode: number | null;
  outcome: RequestAttemptOutcome;
}

export interface RequestTelemetryMetrics {
  queuedRecords: number;
  droppedRecords: number;
  persistedRecords: number;
}

export interface RequestLogList {
  items: RequestLog[];
  telemetry: RequestTelemetryMetrics;
}

export interface RequestLogDetail {
  request: RequestLog;
  attempts: RequestAttempt[];
  telemetry: RequestTelemetryMetrics;
}

export function parseRequestLogList(value: unknown): RequestLogList {
  const record = readRecord(value);
  return {
    items: readArray(record.items).map(parseRequestLog),
    telemetry: parseTelemetry(record.telemetry),
  };
}

export function parseRequestLogDetail(value: unknown): RequestLogDetail {
  const record = readRecord(value);
  return {
    request: parseRequestLog(record.request),
    attempts: readArray(record.attempts).map(parseAttempt),
    telemetry: parseTelemetry(record.telemetry),
  };
}

function parseRequestLog(value: unknown): RequestLog {
  const record = readRecord(value);
  return {
    requestId: readString(record.request_id),
    startedAtMs: readNonNegativeInteger(record.started_at_ms),
    configRevision: readPositiveInteger(record.config_revision),
    gatewayApiKeyId: readNullableString(record.gateway_api_key_id),
    ingressProtocol: readProtocol(record.ingress_protocol),
    operation: readOperation(record.operation),
    publicModel: readNullableString(record.public_model),
    providerEndpointId: readNullableString(record.provider_endpoint_id),
    credentialId: readNullableString(record.credential_id),
    proxyProfileId: readNullableString(record.proxy_profile_id),
    statusCode: readStatusCode(record.status_code),
    errorClass: readNullableEnum(record.error_class, readErrorClass),
    attemptCount: readNonNegativeInteger(record.attempt_count),
    latencyMs: readNonNegativeInteger(record.latency_ms),
    firstTokenMs: readNullableInteger(record.first_token_ms),
    inputTokens: readNullableInteger(record.input_tokens),
    outputTokens: readNullableInteger(record.output_tokens),
    cacheReadTokens: readNullableInteger(record.cache_read_tokens),
    cacheWriteTokens: readNullableInteger(record.cache_write_tokens),
    isStream: readBoolean(record.is_stream),
  };
}

function parseAttempt(value: unknown): RequestAttempt {
  const record = readRecord(value);
  return {
    attemptNo: readPositiveInteger(record.attempt_no),
    routeTargetId: readNullableString(record.route_target_id),
    credentialId: readNullableString(record.credential_id),
    proxyProfileId: readNullableString(record.proxy_profile_id),
    startedAtMs: readNonNegativeInteger(record.started_at_ms),
    durationMs: readNonNegativeInteger(record.duration_ms),
    retrySafety: readNullableEnum(record.retry_safety, readRetrySafety),
    errorClass: readNullableEnum(record.error_class, readErrorClass),
    statusCode: readNullableStatusCode(record.status_code),
    outcome: readOutcome(record.outcome),
  };
}

function parseTelemetry(value: unknown): RequestTelemetryMetrics {
  const record = readRecord(value);
  return {
    queuedRecords: readNonNegativeInteger(record.queued_records),
    droppedRecords: readNonNegativeInteger(record.dropped_records),
    persistedRecords: readNonNegativeInteger(record.persisted_records),
  };
}

function readProtocol(value: unknown): RequestLogProtocol {
  if (
    value === "openai_responses" ||
    value === "codex_backend" ||
    value === "anthropic_messages"
  ) {
    return value;
  }
  throw invalidResponse();
}

function readOperation(value: unknown): RequestLogOperation {
  if (
    value === "responses" ||
    value === "responses_compact" ||
    value === "messages" ||
    value === "messages_count_tokens"
  ) {
    return value;
  }
  throw invalidResponse();
}

function readErrorClass(value: string): RequestLogErrorClass {
  const values: RequestLogErrorClass[] = [
    "invalid_request",
    "authentication",
    "permission_denied",
    "quota_exhausted",
    "rate_limited",
    "model_unavailable",
    "operation_unavailable",
    "proxy",
    "network",
    "upstream",
    "cancelled",
    "internal",
  ];
  if (values.includes(value as RequestLogErrorClass)) {
    return value as RequestLogErrorClass;
  }
  throw invalidResponse();
}

function readRetrySafety(value: string): RequestRetrySafety {
  if (
    value === "definitely_not_sent" ||
    value === "rejected_before_execution" ||
    value === "idempotent" ||
    value === "ambiguous"
  ) {
    return value;
  }
  throw invalidResponse();
}

function readOutcome(value: unknown): RequestAttemptOutcome {
  const values: RequestAttemptOutcome[] = [
    "success",
    "transport_error",
    "upstream_error",
    "invalid_response",
    "local_error",
    "stream_error",
    "cancelled",
  ];
  if (typeof value === "string" && values.includes(value as RequestAttemptOutcome)) {
    return value as RequestAttemptOutcome;
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

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw invalidResponse();
  }
  return value;
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

function readNullableInteger(value: unknown): number | null {
  return value === null ? null : readNonNegativeInteger(value);
}

function readStatusCode(value: unknown): number {
  const status = readNonNegativeInteger(value);
  if (status < 100 || status > 599) {
    throw invalidResponse();
  }
  return status;
}

function readNullableStatusCode(value: unknown): number | null {
  return value === null ? null : readStatusCode(value);
}

function readNullableEnum<T>(value: unknown, parser: (value: string) => T): T | null {
  return value === null ? null : parser(readString(value));
}

function invalidResponse() {
  return new Error("invalid request log response");
}
