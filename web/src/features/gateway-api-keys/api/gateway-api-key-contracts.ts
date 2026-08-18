import { parseRequestUsage, type RequestUsage } from "@/shared/api/request-usage";
import type { GatewayApiKeyCollectionResponse } from "@/shared/api/generated/GatewayApiKeyCollectionResponse";
import type { GatewayApiKeyCreateRequest } from "@/shared/api/generated/GatewayApiKeyCreateRequest";
import type { GatewayApiKeyDeleteRequest } from "@/shared/api/generated/GatewayApiKeyDeleteRequest";
import type { GatewayApiKeyResponse } from "@/shared/api/generated/GatewayApiKeyResponse";
import type { GatewayApiKeyRotateRequest } from "@/shared/api/generated/GatewayApiKeyRotateRequest";
import type { GatewayApiKeyUpdateRequest } from "@/shared/api/generated/GatewayApiKeyUpdateRequest";

// Wire types are generated from the Rust DTOs; parsers below keep
// the semantic assertions (token shape, positive versions, usage coherence)
// that the structural types cannot express.
export type {
  GatewayApiKeyCollectionResponse,
  GatewayApiKeyCreateRequest,
  GatewayApiKeyDeleteRequest,
  GatewayApiKeyRotateRequest,
  GatewayApiKeyUpdateRequest,
};

export interface GatewayApiKey {
  id: string;
  name: string;
  token: string;
  tokenPrefix: string;
  tokenVersion: number;
  configVersion: number;
  enabled: boolean;
  createdAt: string;
  lastUsedAt: string | null;
  usage: RequestUsage;
}

export interface GatewayApiKeyConfiguration {
  configRevision: number;
  items: GatewayApiKey[];
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

export interface GatewayApiKeyDeleteInput {
  expectedRevision: number;
  expectedConfigVersion: number;
}

export function parseGatewayApiKeyConfiguration(
  value: GatewayApiKeyCollectionResponse,
): GatewayApiKeyConfiguration {
  if (!isRecord(value) || !Array.isArray(value.items)) {
    throw new Error("invalid gateway API Key response");
  }
  return {
    configRevision: readPositiveInteger(value.config_revision),
    items: value.items.map(parseGatewayApiKey),
  };
}

function parseGatewayApiKey(value: GatewayApiKeyResponse): GatewayApiKey {
  if (
    !isRecord(value) ||
    "secret" in value ||
    "api_key" in value ||
    "token_hash" in value
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
    createdAt: readString(value.created_at),
    lastUsedAt: readNullableString(value.last_used_at),
    usage: parseRequestUsage(value.usage),
  };
}

function isGatewayToken(value: string) {
  return /^sk-[A-Za-z0-9_-]{43}$/.test(value);
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

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new Error("invalid gateway API Key response");
  }
  return value;
}
