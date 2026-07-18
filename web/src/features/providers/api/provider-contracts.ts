export type ProviderKind = "codex" | "claude";
export type ProtocolDialect = "openai_responses" | "anthropic_messages";

export interface ProviderEndpoint {
  id: string;
  name: string;
  providerKind: ProviderKind;
  baseUrl: string;
  protocolDialect: ProtocolDialect;
  allowInsecureHttp: boolean;
  allowPrivateNetwork: boolean;
  enabled: boolean;
  configVersion: number;
}

export interface ProviderEndpointConfiguration {
  configRevision: number;
  items: ProviderEndpoint[];
}

export interface ProviderEndpointWriteInput {
  expectedRevision: number;
  expectedConfigVersion?: number;
  name: string;
  providerKind: ProviderKind;
  baseUrl: string;
  protocolDialect: ProtocolDialect;
  allowInsecureHttp: boolean;
  allowPrivateNetwork: boolean;
  enabled: boolean;
}

export function parseProviderEndpointConfiguration(
  value: unknown,
): ProviderEndpointConfiguration {
  if (!isRecord(value) || !Array.isArray(value.items)) {
    throw new Error("invalid provider endpoint response");
  }
  return {
    configRevision: readPositiveInteger(value.config_revision),
    items: value.items.map(parseProviderEndpoint),
  };
}

function parseProviderEndpoint(value: unknown): ProviderEndpoint {
  if (!isRecord(value)) {
    throw new Error("invalid provider endpoint response");
  }
  const providerKind = readProviderKind(value.provider_kind);
  const protocolDialect = readProtocolDialect(value.protocol_dialect);
  if (!isCompatible(providerKind, protocolDialect)) {
    throw new Error("invalid provider endpoint response");
  }
  const baseUrl = readString(value.base_url);
  validateBaseUrl(baseUrl);

  return {
    id: readString(value.id),
    name: readString(value.name),
    providerKind,
    baseUrl,
    protocolDialect,
    allowInsecureHttp: readBoolean(value.allow_insecure_http),
    allowPrivateNetwork: readBoolean(value.allow_private_network),
    enabled: readBoolean(value.enabled),
    configVersion: readPositiveInteger(value.config_version),
  };
}

function validateBaseUrl(value: string) {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new Error("invalid provider endpoint response");
  }
  if (
    (url.protocol !== "https:" && url.protocol !== "http:") ||
    url.username.length > 0 ||
    url.password.length > 0 ||
    url.search.length > 0 ||
    url.hash.length > 0
  ) {
    throw new Error("invalid provider endpoint response");
  }
}

function isCompatible(kind: ProviderKind, dialect: ProtocolDialect) {
  return (
    (kind === "codex" && dialect === "openai_responses") ||
    (kind === "claude" && dialect === "anthropic_messages")
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error("invalid provider endpoint response");
  }
  return value;
}

function readPositiveInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) <= 0) {
    throw new Error("invalid provider endpoint response");
  }
  return Number(value);
}

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new Error("invalid provider endpoint response");
  }
  return value;
}

function readProviderKind(value: unknown): ProviderKind {
  if (value !== "codex" && value !== "claude") {
    throw new Error("invalid provider endpoint response");
  }
  return value;
}

function readProtocolDialect(value: unknown): ProtocolDialect {
  if (value !== "openai_responses" && value !== "anthropic_messages") {
    throw new Error("invalid provider endpoint response");
  }
  return value;
}
