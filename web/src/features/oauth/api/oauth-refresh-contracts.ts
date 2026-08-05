export type OAuthRefreshTrigger = "scheduled" | "authentication_failure";
export type OAuthRefreshFailureStage =
  | "preflight"
  | "request_build"
  | "dns"
  | "tcp"
  | "proxy_handshake"
  | "tls"
  | "write_request"
  | "await_headers"
  | "read_response"
  | "token_endpoint"
  | "parse_response"
  | "validate_token"
  | "publish_token"
  | "verify_authentication";
export type OAuthRefreshFailureReason =
  | "account_unavailable"
  | "provider_unavailable"
  | "token_material_unavailable"
  | "proxy_unavailable"
  | "refresh_token_missing"
  | "request_invalid"
  | "transport_failure"
  | "read_timeout"
  | "response_too_large"
  | "invalid_grant"
  | "refresh_token_expired"
  | "refresh_token_reused"
  | "refresh_token_invalidated"
  | "upstream_rejected"
  | "invalid_response"
  | "provider_mismatch"
  | "routing_profile_invalid"
  | "document_serialization_failed"
  | "publication_conflict"
  | "publication_failed"
  | "refresh_unavailable"
  | "refreshed_access_token_rejected";
export type OAuthRefreshFailureScope = "endpoint" | "proxy" | "unattributed";

export interface OAuthRefreshFailure {
  tokenVersion: number;
  trigger: OAuthRefreshTrigger;
  stage: OAuthRefreshFailureStage;
  reason: OAuthRefreshFailureReason;
  upstreamStatus: number | null;
  failureScope: OAuthRefreshFailureScope | null;
  occurredAt: number;
  reauthorizationRequired: boolean;
}

export function parseOAuthRefreshFailure(value: unknown): OAuthRefreshFailure | null {
  if (value === null) {
    return null;
  }
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const upstreamStatus = readOptionalInteger(value.upstream_status, 100);
  if (upstreamStatus !== null && upstreamStatus > 599) {
    throw invalidResponse();
  }
  return {
    tokenVersion: readInteger(value.token_version, 1),
    trigger: readRefreshTrigger(value.trigger),
    stage: readRefreshStage(value.stage),
    reason: readRefreshReason(value.reason),
    upstreamStatus,
    failureScope: readRefreshFailureScope(value.failure_scope),
    occurredAt: readInteger(value.occurred_at, 0),
    reauthorizationRequired: readBoolean(value.reauthorization_required),
  };
}

function readRefreshTrigger(value: unknown): OAuthRefreshTrigger {
  if (value !== "scheduled" && value !== "authentication_failure") {
    throw invalidResponse();
  }
  return value;
}

const REFRESH_STAGES = new Set<OAuthRefreshFailureStage>([
  "preflight",
  "request_build",
  "dns",
  "tcp",
  "proxy_handshake",
  "tls",
  "write_request",
  "await_headers",
  "read_response",
  "token_endpoint",
  "parse_response",
  "validate_token",
  "publish_token",
  "verify_authentication",
]);

function readRefreshStage(value: unknown): OAuthRefreshFailureStage {
  if (typeof value !== "string" || !REFRESH_STAGES.has(value as OAuthRefreshFailureStage)) {
    throw invalidResponse();
  }
  return value as OAuthRefreshFailureStage;
}

const REFRESH_REASONS = new Set<OAuthRefreshFailureReason>([
  "account_unavailable",
  "provider_unavailable",
  "token_material_unavailable",
  "proxy_unavailable",
  "refresh_token_missing",
  "request_invalid",
  "transport_failure",
  "read_timeout",
  "response_too_large",
  "invalid_grant",
  "refresh_token_expired",
  "refresh_token_reused",
  "refresh_token_invalidated",
  "upstream_rejected",
  "invalid_response",
  "provider_mismatch",
  "routing_profile_invalid",
  "document_serialization_failed",
  "publication_conflict",
  "publication_failed",
  "refresh_unavailable",
  "refreshed_access_token_rejected",
]);

function readRefreshReason(value: unknown): OAuthRefreshFailureReason {
  if (typeof value !== "string" || !REFRESH_REASONS.has(value as OAuthRefreshFailureReason)) {
    throw invalidResponse();
  }
  return value as OAuthRefreshFailureReason;
}

function readRefreshFailureScope(value: unknown): OAuthRefreshFailureScope | null {
  if (value === null) {
    return null;
  }
  if (value !== "endpoint" && value !== "proxy" && value !== "unattributed") {
    throw invalidResponse();
  }
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readInteger(value: unknown, minimum: number) {
  if (typeof value !== "number" || !Number.isInteger(value) || value < minimum) {
    throw invalidResponse();
  }
  return value;
}

function readOptionalInteger(value: unknown, minimum: number) {
  return value === null ? null : readInteger(value, minimum);
}

function readBoolean(value: unknown) {
  if (typeof value !== "boolean") {
    throw invalidResponse();
  }
  return value;
}

function invalidResponse() {
  return new Error("invalid OAuth2 login response");
}
