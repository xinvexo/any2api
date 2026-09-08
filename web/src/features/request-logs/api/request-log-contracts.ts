import type { ActiveRequestLogResponse } from "@/shared/api/generated/ActiveRequestLogResponse";
import type { RequestLogListResponse } from "@/shared/api/generated/RequestLogListResponse";
import type { RequestLogDetailResponse } from "@/shared/api/generated/RequestLogDetailResponse";
import type { RequestLogResponse } from "@/shared/api/generated/RequestLogResponse";
import type { RequestQuotaCostResponse } from "@/shared/api/generated/RequestQuotaCostResponse";
import type { RequestTelemetryResponse } from "@/shared/api/generated/RequestTelemetryResponse";

import {
  parseRequestAttempt,
  parseRequestLogOutcome,
  type RequestAttempt,
  type RequestLogOutcome,
} from "./request-attempt-contracts";
import {
  parseRequestLogFilterOptions,
  type RequestLogFilterOptions,
  type RequestLogOperation,
} from "./request-log-filter-contracts";
import {
  isProtocolDialect,
  isProtocolOperation,
  type ProtocolDialect,
} from "@/shared/api/provider-protocol-vocabulary";

export type {
  RequestAttempt,
  RequestAttemptFailureScope,
  RequestAttemptRetryDecision,
  RequestAttemptStreamTiming,
  RequestAttemptTransport,
  RequestLogOutcome,
  RequestRoutingMode,
  RequestTransportResolverMode,
  RequestTransportTrafficClass,
} from "./request-attempt-contracts";
export type {
  RequestLogFilterOptions,
  RequestLogFilters,
  RequestLogOperation,
  StableRequestLogFilterOption,
} from "./request-log-filter-contracts";

export type RequestLogProtocol = ProtocolDialect;
export type RequestSpeedTier = "standard" | "fast";
export interface RequestQuotaCost {
  unit: "codex_credits";
  amountNanos: string;
  rateCard: string;
  serviceTier: RequestSpeedTier;
  creditsPerUsd: number | null;
}

export interface RequestLog {
  requestId: string;
  startedAtMs: number;
  clientIp: string;
  configRevision: number;
  gatewayApiKeyId: string | null;
  ingressProtocol: RequestLogProtocol;
  operation: RequestLogOperation;
  publicModel: string | null;
  thinkingLevel: string | null;
  providerEndpointId: string | null;
  providerEndpointName: string | null;
  credentialId: string | null;
  credentialLabel: string | null;
  oauthAccountId: string | null;
  oauthAccountLabel: string | null;
  proxyProfileId: string | null;
  proxyProfileLabel: string | null;
  statusCode: number;
  outcome: RequestLogOutcome;
  errorMessage: string | null;
  attemptCount: number;
  latencyMs: number;
  firstTokenMs: number | null;
  inputTokens: number | null;
  outputTokens: number | null;
  cacheReadTokens: number | null;
  cacheCreationTokens: number | null;
  quotaCost: RequestQuotaCost | null;
  isStream: boolean;
  requestedSpeedTier: RequestSpeedTier | null;
  effectiveSpeedTier: RequestSpeedTier | null;
}

export interface ActiveRequestLog {
  state: "processing";
  requestId: string;
  startedAtMs: number;
  clientIp: string;
  configRevision: number;
  gatewayApiKeyId: string;
  ingressProtocol: RequestLogProtocol;
  operation: RequestLogOperation;
  publicModel: string | null;
  thinkingLevel: string | null;
  providerEndpointId: string | null;
  providerEndpointName: string | null;
  credentialId: string | null;
  credentialLabel: string | null;
  oauthAccountId: string | null;
  oauthAccountLabel: string | null;
  proxyProfileId: string | null;
  proxyProfileLabel: string | null;
  attemptCount: number;
  isStream: boolean | null;
  requestedSpeedTier: RequestSpeedTier | null;
  effectiveSpeedTier: RequestSpeedTier | null;
}

interface RequestTelemetryMetrics {
  queuedRecords: number;
  inFlightRecords: number;
  droppedRecords: number;
  persistedRecords: number;
}

export interface RequestLogList {
  activeItems: ActiveRequestLog[];
  activeTotal: number;
  items: RequestLog[];
  nextCursor: string | null;
  hasMore: boolean;
  telemetry: RequestTelemetryMetrics;
  filterOptions: RequestLogFilterOptions;
}

export interface RequestLogDetail {
  request: RequestLog;
  attempts: RequestAttempt[];
  telemetry: RequestTelemetryMetrics;
}

export function parseRequestLogList(value: unknown): RequestLogList {
  const record = readRecord<RequestLogListResponse>(value);
  const activeItems = readArray(record.active_items).map(parseActiveRequestLog);
  const activeTotal = readNonNegativeInteger(record.active_total);
  const items = readArray(record.items).map(parseRequestLog);
  const nextCursor = readCursor(record.next_cursor);
  const hasMore = readBoolean(record.has_more);
  if (
    activeItems.length > 100 ||
    activeItems.length > activeTotal ||
    items.length > 100 ||
    hasMore !== (nextCursor !== null)
  ) {
    throw invalidResponse();
  }
  return {
    activeItems,
    activeTotal,
    items,
    nextCursor,
    hasMore,
    telemetry: parseTelemetry(record.telemetry),
    filterOptions: parseRequestLogFilterOptions(record.filter_options),
  };
}

function parseActiveRequestLog(value: unknown): ActiveRequestLog {
  const record = readRecord<ActiveRequestLogResponse>(value);
  if (record.state !== "processing") {
    throw invalidResponse();
  }
  return {
    state: "processing",
    requestId: readString(record.request_id),
    startedAtMs: readNonNegativeInteger(record.started_at_ms),
    clientIp: readString(record.client_ip),
    configRevision: readPositiveInteger(record.config_revision),
    gatewayApiKeyId: readString(record.gateway_api_key_id),
    ingressProtocol: readProtocol(record.ingress_protocol),
    operation: readOperation(record.operation),
    publicModel: readNullableString(record.public_model),
    thinkingLevel: readNullableDisplayString(record.thinking_level),
    providerEndpointId: readNullableString(record.provider_endpoint_id),
    providerEndpointName: readNullableDisplayString(record.provider_endpoint_name),
    credentialId: readNullableString(record.credential_id),
    credentialLabel: readNullableDisplayString(record.credential_label),
    oauthAccountId: readNullableString(record.oauth_account_id),
    oauthAccountLabel: readNullableDisplayString(record.oauth_account_label),
    proxyProfileId: readNullableString(record.proxy_profile_id),
    proxyProfileLabel: readNullableDisplayString(record.proxy_profile_label),
    attemptCount: readNonNegativeInteger(record.attempt_count),
    isStream: readNullableBoolean(record.is_stream),
    requestedSpeedTier: readNullableSpeedTier(record.requested_speed_tier),
    effectiveSpeedTier: readNullableSpeedTier(record.effective_speed_tier),
  };
}

export function parseRequestLogDetail(value: unknown): RequestLogDetail {
  const record = readRecord<RequestLogDetailResponse>(value);
  return {
    request: parseRequestLog(record.request),
    attempts: readArray(record.attempts).map(parseRequestAttempt),
    telemetry: parseTelemetry(record.telemetry),
  };
}

function parseRequestLog(value: unknown): RequestLog {
  const record = readRecord<RequestLogResponse>(value);
  const statusCode = readStatusCode(record.status_code);
  const outcome = parseRequestLogOutcome(record.outcome);
  if (outcome === "success" && (statusCode < 200 || statusCode >= 300)) {
    throw invalidResponse();
  }
  return {
    requestId: readString(record.request_id),
    startedAtMs: readNonNegativeInteger(record.started_at_ms),
    clientIp: readString(record.client_ip),
    configRevision: readPositiveInteger(record.config_revision),
    gatewayApiKeyId: readNullableString(record.gateway_api_key_id),
    ingressProtocol: readProtocol(record.ingress_protocol),
    operation: readOperation(record.operation),
    publicModel: readNullableString(record.public_model),
    thinkingLevel: readNullableDisplayString(record.thinking_level),
    providerEndpointId: readNullableString(record.provider_endpoint_id),
    providerEndpointName: readNullableDisplayString(record.provider_endpoint_name),
    credentialId: readNullableString(record.credential_id),
    credentialLabel: readNullableDisplayString(record.credential_label),
    oauthAccountId: readNullableString(record.oauth_account_id),
    oauthAccountLabel: readNullableDisplayString(record.oauth_account_label),
    proxyProfileId: readNullableString(record.proxy_profile_id),
    proxyProfileLabel: readNullableDisplayString(record.proxy_profile_label),
    statusCode,
    outcome,
    errorMessage: readNullableDisplayString(record.error_message),
    attemptCount: readNonNegativeInteger(record.attempt_count),
    latencyMs: readNonNegativeInteger(record.latency_ms),
    firstTokenMs: readNullableInteger(record.first_token_ms),
    inputTokens: readNullableInteger(record.input_tokens),
    outputTokens: readNullableInteger(record.output_tokens),
    cacheReadTokens: readNullableInteger(record.cache_read_tokens),
    cacheCreationTokens: readNullableInteger(record.cache_creation_tokens),
    quotaCost: parseRequestQuotaCost(record.quota_cost),
    isStream: readBoolean(record.is_stream),
    requestedSpeedTier: readNullableSpeedTier(record.requested_speed_tier),
    effectiveSpeedTier: readNullableSpeedTier(record.effective_speed_tier),
  };
}

function parseRequestQuotaCost(value: unknown): RequestQuotaCost | null {
  if (value === null) {
    return null;
  }
  const record = readRecord<RequestQuotaCostResponse>(value);
  if (record.unit !== "codex_credits") {
    throw invalidResponse();
  }
  return {
    unit: "codex_credits",
    amountNanos: readNonNegativeDecimalString(record.amount_nanos),
    rateCard: readString(record.rate_card),
    serviceTier: readSpeedTier(record.service_tier),
    creditsPerUsd: readNullablePositiveInteger(record.credits_per_usd),
  };
}

function parseTelemetry(value: unknown): RequestTelemetryMetrics {
  const record = readRecord<RequestTelemetryResponse>(value);
  return {
    queuedRecords: readNonNegativeInteger(record.queued_records),
    inFlightRecords: readNonNegativeInteger(record.in_flight_records),
    droppedRecords: readNonNegativeInteger(record.dropped_records),
    persistedRecords: readNonNegativeInteger(record.persisted_records),
  };
}

function readProtocol(value: unknown): RequestLogProtocol {
  if (isProtocolDialect(value)) {
    return value;
  }
  throw invalidResponse();
}

function readOperation(value: unknown): RequestLogOperation {
  if (isProtocolOperation(value)) {
    return value;
  }
  throw invalidResponse();
}

function readRecord<T extends object>(value: unknown): T {
  if (typeof value !== "object" || value === null) {
    throw invalidResponse();
  }
  return value as T;
}

function readArray(value: unknown): unknown[] {
  if (!Array.isArray(value)) {
    throw invalidResponse();
  }
  return value;
}

function readString(value: string): string {
  if (typeof value !== "string" || value.length === 0) {
    throw invalidResponse();
  }
  return value;
}

function readNullableString(value: string | null): string | null {
  return value === null ? null : readString(value);
}

function readCursor(value: string | null): string | null {
  const cursor = readNullableString(value);
  if (cursor !== null && cursor.length > 1_024) {
    throw invalidResponse();
  }
  return cursor;
}

function readNullableDisplayString(value: string | null): string | null {
  if (value === null) {
    return null;
  }
  if (typeof value !== "string") {
    throw invalidResponse();
  }
  const trimmed = value.trim();
  return trimmed.length === 0 ? null : trimmed;
}

function readBoolean(value: boolean): boolean {
  if (typeof value !== "boolean") {
    throw invalidResponse();
  }
  return value;
}

function readNullableBoolean(value: boolean | null): boolean | null {
  return value === null ? null : readBoolean(value);
}

function readNullableSpeedTier(value: unknown): RequestSpeedTier | null {
  if (value === null || value === "standard" || value === "fast") {
    return value;
  }
  throw invalidResponse();
}

function readSpeedTier(value: unknown): RequestSpeedTier {
  const tier = readNullableSpeedTier(value);
  if (tier === null) {
    throw invalidResponse();
  }
  return tier;
}

function readNonNegativeDecimalString(value: string): string {
  if (typeof value !== "string" || !/^(?:0|[1-9]\d*)$/u.test(value)) {
    throw invalidResponse();
  }
  return value;
}

function readNonNegativeInteger(value: number): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw invalidResponse();
  }
  return value;
}

function readPositiveInteger(value: number): number {
  const number = readNonNegativeInteger(value);
  if (number === 0) {
    throw invalidResponse();
  }
  return number;
}

function readNullablePositiveInteger(value: number | null): number | null {
  return value === null ? null : readPositiveInteger(value);
}

function readNullableInteger(value: number | null): number | null {
  return value === null ? null : readNonNegativeInteger(value);
}

function readStatusCode(value: number): number {
  const status = readNonNegativeInteger(value);
  if (status < 100 || status > 599) {
    throw invalidResponse();
  }
  return status;
}

function invalidResponse() {
  return new Error("invalid request log response");
}
