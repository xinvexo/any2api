export interface GatewayApiKey {
  id: string;
  name: string;
  tokenPrefix: string;
  tokenVersion: number;
  configVersion: number;
  enabled: boolean;
  revokedAt: string | null;
  createdAt: string;
  lastUsedAt: string | null;
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
  if (!isRecord(value) || "token" in value || !Array.isArray(value.items)) {
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
  const configurationValue = { ...value };
  delete configurationValue.token;
  return {
    configuration: parseGatewayApiKeyConfiguration(configurationValue),
    token: value.token,
  };
}

function parseGatewayApiKey(value: unknown): GatewayApiKey {
  if (
    !isRecord(value) ||
    "secret" in value ||
    "api_key" in value ||
    "token" in value ||
    "token_hash" in value ||
    "ciphertext" in value
  ) {
    throw new Error("invalid gateway API Key response");
  }
  return {
    id: readString(value.id),
    name: readString(value.name),
    tokenPrefix: readVisibleAscii(value.token_prefix),
    tokenVersion: readPositiveInteger(value.token_version),
    configVersion: readPositiveInteger(value.config_version),
    enabled: readBoolean(value.enabled),
    revokedAt: readNullableString(value.revoked_at),
    createdAt: readString(value.created_at),
    lastUsedAt: readNullableString(value.last_used_at),
  };
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
  if (![...parsed].every((character) => {
    const code = character.charCodeAt(0);
    return code >= 0x21 && code <= 0x7e;
  })) {
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

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new Error("invalid gateway API Key response");
  }
  return value;
}
