export type RequestLogOutcome = "success" | "failed" | "cancelled";
export type RequestRoutingMode = "balanced" | "bound";
export type RequestAttemptFailureScope =
  | "unattributed"
  | "authentication"
  | "credential"
  | "credential_model"
  | "route_operation"
  | "exact_candidate"
  | "egress_path"
  | "proxy"
  | "endpoint";
export type RequestAttemptRetryDecision =
  | "terminal"
  | "oauth_refresh"
  | "retry_same_path"
  | "reselect";
export type RequestTransportResolverMode = "system" | "proxy_remote" | "local_cached";
export type RequestTransportTrafficClass =
  | "data_plane"
  | "oauth_token"
  | "oauth_quota"
  | "diagnostic";

export interface RequestAttemptTransport {
  wireProfileId: string;
  wireProfileVersion: number;
  timeoutPolicyVersion: number;
  resolverMode: RequestTransportResolverMode;
  proxyKind: "direct" | "http" | "socks5";
  connectTimeoutMs: number;
  readTimeoutMs: number;
  poolIdleTimeoutMs: number;
  routingGeneration: number;
  authenticationVersion: number;
  trafficClass: RequestTransportTrafficClass;
}

export interface RequestAttemptStreamTiming {
  firstUpstreamFrameMs: number | null;
  streamCommitMs: number | null;
  firstDownstreamByteMs: number | null;
  streamCancelMs: number | null;
}

export interface RequestAttempt {
  attemptNo: number;
  routeTargetId: string | null;
  credentialId: string | null;
  credentialLabel: string | null;
  oauthAccountId: string | null;
  oauthAccountLabel: string | null;
  proxyProfileId: string | null;
  proxyProfileLabel: string | null;
  routingMode: RequestRoutingMode | null;
  failureScope: RequestAttemptFailureScope | null;
  retryDecision: RequestAttemptRetryDecision | null;
  startedAtMs: number;
  durationMs: number;
  errorMessage: string | null;
  statusCode: number | null;
  outcome: RequestLogOutcome;
  transport: RequestAttemptTransport | null;
  streamTiming: RequestAttemptStreamTiming | null;
}

export function parseRequestAttempt(value: unknown): RequestAttempt {
  const record = readRecord(value);
  const statusCode = readNullableStatusCode(record.status_code);
  const outcome = parseRequestLogOutcome(record.outcome);
  if (outcome === "success" && (statusCode === null || statusCode < 200 || statusCode >= 300)) {
    throw invalidResponse();
  }
  return {
    attemptNo: readPositiveInteger(record.attempt_no),
    routeTargetId: readNullableString(record.route_target_id),
    credentialId: readNullableString(record.credential_id),
    credentialLabel: readNullableDisplayString(record.credential_label),
    oauthAccountId: readNullableString(record.oauth_account_id),
    oauthAccountLabel: readNullableDisplayString(record.oauth_account_label),
    proxyProfileId: readNullableString(record.proxy_profile_id),
    proxyProfileLabel: readNullableDisplayString(record.proxy_profile_label),
    routingMode: readNullableRoutingMode(record.routing_mode),
    failureScope: readNullableFailureScope(record.failure_scope),
    retryDecision: readNullableRetryDecision(record.retry_decision),
    startedAtMs: readNonNegativeInteger(record.started_at_ms),
    durationMs: readNonNegativeInteger(record.duration_ms),
    errorMessage: readNullableDisplayString(record.error_message),
    statusCode,
    outcome,
    transport: readNullable(record.transport, parseTransport),
    streamTiming: readNullable(record.stream_timing, parseStreamTiming),
  };
}

export function parseRequestLogOutcome(value: unknown): RequestLogOutcome {
  if (value === "success" || value === "failed" || value === "cancelled") {
    return value;
  }
  throw invalidResponse();
}

function parseTransport(value: unknown): RequestAttemptTransport {
  const record = readRecord(value);
  const wireProfileId = readString(record.wire_profile_id);
  if (wireProfileId.length > 64) {
    throw invalidResponse();
  }
  return {
    wireProfileId,
    wireProfileVersion: readPositiveInteger(record.wire_profile_version),
    timeoutPolicyVersion: readPositiveInteger(record.timeout_policy_version),
    resolverMode: readResolverMode(record.resolver_mode),
    proxyKind: readProxyKind(record.proxy_kind),
    connectTimeoutMs: readNonNegativeInteger(record.connect_timeout_ms),
    readTimeoutMs: readNonNegativeInteger(record.read_timeout_ms),
    poolIdleTimeoutMs: readNonNegativeInteger(record.pool_idle_timeout_ms),
    routingGeneration: readPositiveInteger(record.routing_generation),
    authenticationVersion: readPositiveInteger(record.authentication_version),
    trafficClass: readTrafficClass(record.traffic_class),
  };
}

function parseStreamTiming(value: unknown): RequestAttemptStreamTiming {
  const record = readRecord(value);
  const timing = {
    firstUpstreamFrameMs: readNullableInteger(record.first_upstream_frame_ms),
    streamCommitMs: readNullableInteger(record.stream_commit_ms),
    firstDownstreamByteMs: readNullableInteger(record.first_downstream_byte_ms),
    streamCancelMs: readNullableInteger(record.stream_cancel_ms),
  };
  if (Object.values(timing).every((item) => item === null)) {
    throw invalidResponse();
  }
  return timing;
}

function readResolverMode(value: unknown): RequestTransportResolverMode {
  if (value === "system" || value === "proxy_remote" || value === "local_cached") {
    return value;
  }
  throw invalidResponse();
}

function readTrafficClass(value: unknown): RequestTransportTrafficClass {
  if (
    value === "data_plane" ||
    value === "oauth_token" ||
    value === "oauth_quota" ||
    value === "diagnostic"
  ) {
    return value;
  }
  throw invalidResponse();
}

function readProxyKind(value: unknown): RequestAttemptTransport["proxyKind"] {
  if (value === "direct" || value === "http" || value === "socks5") {
    return value;
  }
  throw invalidResponse();
}

function readNullableRoutingMode(value: unknown): RequestRoutingMode | null {
  if (value === null || value === "balanced" || value === "bound") {
    return value;
  }
  throw invalidResponse();
}

function readNullableFailureScope(value: unknown): RequestAttemptFailureScope | null {
  const values: RequestAttemptFailureScope[] = [
    "unattributed", "authentication", "credential", "credential_model", "route_operation",
    "exact_candidate", "egress_path", "proxy", "endpoint",
  ];
  if (value === null || values.includes(value as RequestAttemptFailureScope)) {
    return value as RequestAttemptFailureScope | null;
  }
  throw invalidResponse();
}

function readNullableRetryDecision(value: unknown): RequestAttemptRetryDecision | null {
  const values: RequestAttemptRetryDecision[] = [
    "terminal", "oauth_refresh", "retry_same_path", "reselect",
  ];
  if (value === null || values.includes(value as RequestAttemptRetryDecision)) {
    return value as RequestAttemptRetryDecision | null;
  }
  throw invalidResponse();
}

function readRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null) throw invalidResponse();
  return value as Record<string, unknown>;
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) throw invalidResponse();
  return value;
}

function readNullableString(value: unknown): string | null {
  return value === null ? null : readString(value);
}

function readNullableDisplayString(value: unknown): string | null {
  if (value === null) return null;
  if (typeof value !== "string") throw invalidResponse();
  return value.trim() || null;
}

function readNonNegativeInteger(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw invalidResponse();
  }
  return value;
}

function readPositiveInteger(value: unknown): number {
  const number = readNonNegativeInteger(value);
  if (number === 0) throw invalidResponse();
  return number;
}

function readNullableInteger(value: unknown): number | null {
  return value === null ? null : readNonNegativeInteger(value);
}

function readNullableStatusCode(value: unknown): number | null {
  if (value === null) return null;
  const status = readNonNegativeInteger(value);
  if (status < 100 || status > 599) throw invalidResponse();
  return status;
}

function readNullable<T>(value: unknown, parse: (value: unknown) => T): T | null {
  return value === null ? null : parse(value);
}

function invalidResponse() {
  return new Error("invalid request log response");
}
