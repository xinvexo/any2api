export type AffinityBindingKind = "soft" | "hard";
export type AffinityCredentialSource = "provider_credential" | "oauth_account";
export type AffinityProtocolDialect =
  | "openai_responses"
  | "codex_backend"
  | "anthropic_messages";

export interface AffinityCredentialCount {
  credentialId: string;
  credentialSource: AffinityCredentialSource;
  credentialLabel: string;
  softBindings: number;
  hardBindings: number;
}

export interface AffinityBinding {
  kind: AffinityBindingKind;
  sessionHashPrefix: string;
  credentialId: string;
  credentialSource: AffinityCredentialSource;
  routeTargetId: string;
  upstreamModel: string;
  protocolDialect: AffinityProtocolDialect;
  expiresInMs: number;
}

export interface AffinityRuntime {
  configRevision: number;
  softBindingCount: number;
  hardBindingCount: number;
  creatingCount: number;
  credentialCounts: AffinityCredentialCount[];
  bindings: AffinityBinding[];
}

export interface AffinityClearResult {
  clearedCount: number;
}

export function parseAffinityRuntime(value: unknown): AffinityRuntime {
  const record = readRecord(value);
  return {
    configRevision: readPositiveInteger(record.config_revision),
    softBindingCount: readNonNegativeInteger(record.soft_binding_count),
    hardBindingCount: readNonNegativeInteger(record.hard_binding_count),
    creatingCount: readNonNegativeInteger(record.creating_count),
    credentialCounts: readArray(record.credential_counts).map(parseCredentialCount),
    bindings: readArray(record.bindings).map(parseBinding),
  };
}

export function parseAffinityClearResult(value: unknown): AffinityClearResult {
  const record = readRecord(value);
  return { clearedCount: readNonNegativeInteger(record.cleared_count) };
}

function parseCredentialCount(value: unknown): AffinityCredentialCount {
  const record = readRecord(value);
  return {
    credentialId: readString(record.credential_id),
    credentialSource: readCredentialSource(record.credential_source),
    credentialLabel: readString(record.credential_label),
    softBindings: readNonNegativeInteger(record.soft_bindings),
    hardBindings: readNonNegativeInteger(record.hard_bindings),
  };
}

function parseBinding(value: unknown): AffinityBinding {
  const record = readRecord(value);
  return {
    kind: readBindingKind(record.kind),
    sessionHashPrefix: readString(record.session_hash_prefix),
    credentialId: readString(record.credential_id),
    credentialSource: readCredentialSource(record.credential_source),
    routeTargetId: readString(record.route_target_id),
    upstreamModel: readString(record.upstream_model),
    protocolDialect: readProtocolDialect(record.protocol_dialect),
    expiresInMs: readNonNegativeInteger(record.expires_in_ms),
  };
}

function readRecord(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null) {
    throw new Error("invalid affinity response");
  }
  return value as Record<string, unknown>;
}

function readArray(value: unknown): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error("invalid affinity response");
  }
  return value;
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error("invalid affinity response");
  }
  return value;
}

function readPositiveInteger(value: unknown): number {
  const number = readNonNegativeInteger(value);
  if (number === 0) {
    throw new Error("invalid affinity response");
  }
  return number;
}

function readNonNegativeInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) < 0) {
    throw new Error("invalid affinity response");
  }
  return Number(value);
}

function readBindingKind(value: unknown): AffinityBindingKind {
  if (value !== "soft" && value !== "hard") {
    throw new Error("invalid affinity response");
  }
  return value;
}

function readCredentialSource(value: unknown): AffinityCredentialSource {
  if (value !== "provider_credential" && value !== "oauth_account") {
    throw new Error("invalid affinity response");
  }
  return value;
}

function readProtocolDialect(value: unknown): AffinityProtocolDialect {
  if (
    value !== "openai_responses" &&
    value !== "codex_backend" &&
    value !== "anthropic_messages"
  ) {
    throw new Error("invalid affinity response");
  }
  return value;
}
