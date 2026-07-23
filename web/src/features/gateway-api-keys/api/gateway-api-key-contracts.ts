export interface GatewayApiKey {
  id: string;
  name: string;
  token: string;
  tokenPrefix: string;
  tokenVersion: number;
  configVersion: number;
  enabled: boolean;
  revokedAt: string | null;
  createdAt: string;
  lastUsedAt: string | null;
  usage: GatewayApiKeyUsage;
}

export interface GatewayApiKeyUsage {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  recentOutcomes: GatewayApiKeyRequestOutcome[];
}

export interface GatewayApiKeyRequestOutcome {
  statusCode: number;
}

export interface GatewayApiKeyConfiguration {
  configRevision: number;
  items: GatewayApiKey[];
}

export interface GatewayApiKeySecretReceipt {
  configuration: GatewayApiKeyConfiguration;
  token: string;
}

export interface GatewayApiKeyCreateInput {
  expectedRevision: number;
  name: string;
  enabled: boolean;
}

export interface GatewayApiKeyUpdateInput {
  expectedRevision: number;
  expectedConfigVersion: number;
  name: string;
  enabled: boolean;
}

export interface GatewayApiKeyRotateInput {
  expectedRevision: number;
  expectedConfigVersion: number;
  expectedTokenVersion: number;
}

export interface GatewayApiKeyRevokeInput {
  expectedRevision: number;
  expectedConfigVersion: number;
}

export function parseGatewayApiKeyConfiguration(value: unknown): GatewayApiKeyConfiguration {
  if (!isRecord(value) || !Array.isArray(value.items)) {
    throw new Error("invalid gateway API Key response");
  }
  return {
    configRevision: readPositiveInteger(value.config_revision),
    items: value.items.map(parseGatewayApiKey),
  };
}

export function parseGatewayApiKeySecretReceipt(value: unknown): GatewayApiKeySecretReceipt {
  if (!isRecord(value) || typeof value.token !== "string" || !isGatewayToken(value.token)) {
    throw new Error("invalid gateway API Key secret receipt");
  }
  return {
    configuration: parseGatewayApiKeyConfiguration({
      config_revision: value.config_revision,
      items: value.items,
    }),
    token: value.token,
  };
}

function parseGatewayApiKey(value: unknown): GatewayApiKey {
  if (
    !isRecord(value) ||
    "secret" in value ||
    "api_key" in value ||
    "token_hash" in value ||
    "ciphertext" in value
  ) {
    throw new Error("invalid gateway API Key response");
  }
  const token = readString(value.token);
  if (!isGatewayToken(token)) {
    throw new Error("invalid gateway API Key response");
  }
  return {
    id: readString(value.id),
    name: readString(value.name),
    token,
    tokenPrefix: readVisibleAscii(value.token_prefix),
    tokenVersion: readPositiveInteger(value.token_version),
    configVersion: readPositiveInteger(value.config_version),
    enabled: readBoolean(value.enabled),
    revokedAt: readNullableString(value.revoked_at),
    createdAt: readString(value.created_at),
    lastUsedAt: readNullableString(value.last_used_at),
    usage: parseGatewayApiKeyUsage(value.usage),
  };
}

function parseGatewayApiKeyUsage(value: unknown): GatewayApiKeyUsage {
  if (!isRecord(value) || !Array.isArray(value.recent_outcomes)) {
    throw new Error("invalid gateway API Key response");
  }
  const totalRequests = readNonNegativeInteger(value.total_requests);
  const successfulRequests = readNonNegativeInteger(value.successful_requests);
  const failedRequests = readNonNegativeInteger(value.failed_requests);
  if (
    successfulRequests > totalRequests ||
    failedRequests > totalRequests ||
    successfulRequests + failedRequests !== totalRequests
  ) {
    throw new Error("invalid gateway API Key response");
  }
  return {
    totalRequests,
    successfulRequests,
    failedRequests,
    recentOutcomes: value.recent_outcomes.map(parseGatewayApiKeyRequestOutcome),
  };
}

function parseGatewayApiKeyRequestOutcome(value: unknown): GatewayApiKeyRequestOutcome {
  if (!isRecord(value)) {
    throw new Error("invalid gateway API Key response");
  }
  const statusCode = readNonNegativeInteger(value.status_code);
  if (statusCode < 100 || statusCode > 599) {
    throw new Error("invalid gateway API Key response");
  }
  return { statusCode };
}

function isGatewayToken(value: string) {
  return /^a2k_v1_[A-Za-z0-9_-]{43}$/.test(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error("invalid gateway API Key response");
  }
  return value;
}

function readVisibleAscii(value: unknown): string {
  const parsed = readString(value);
  if (
    ![...parsed].every((character) => {
      const code = character.charCodeAt(0);
      return code >= 0x21 && code <= 0x7e;
    })
  ) {
    throw new Error("invalid gateway API Key response");
  }
  return parsed;
}

function readNullableString(value: unknown): string | null {
  return value === null ? null : readString(value);
}

function readPositiveInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) <= 0) {
    throw new Error("invalid gateway API Key response");
  }
  return Number(value);
}

function readNonNegativeInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) < 0) {
    throw new Error("invalid gateway API Key response");
  }
  return Number(value);
}

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new Error("invalid gateway API Key response");
  }
  return value;
}
