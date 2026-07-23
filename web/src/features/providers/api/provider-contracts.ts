export type ProviderKind = "codex" | "claude";
export type ProtocolDialect =
  | "openai_responses"
  | "openai_chat_completions"
  | "codex_backend"
  | "anthropic_messages";

export interface ProviderProtocolOptions {
  providerKind: ProviderKind;
  acceptedProtocol: ProtocolDialect;
  upstreamProtocols: ProtocolDialect[];
}

export interface ProviderEndpoint {
  id: string;
  name: string;
  providerKind: ProviderKind;
  baseUrl: string;
  protocolDialect: ProtocolDialect;
  upstreamProtocolDialect: ProtocolDialect | null;
  enabled: boolean;
  configVersion: number;
}

export interface ProviderEndpointConfiguration {
  configRevision: number;
  items: ProviderEndpoint[];
  protocolOptions: ProviderProtocolOptions[];
}

export interface ProviderEndpointWriteInput {
  expectedRevision: number;
  expectedConfigVersion?: number;
  name: string;
  providerKind: ProviderKind;
  baseUrl: string;
  protocolDialect: ProtocolDialect;
  upstreamProtocolDialect: ProtocolDialect | null;
  enabled: boolean;
}

export function parseProviderEndpointConfiguration(
  value: unknown,
): ProviderEndpointConfiguration {
  if (
    !isRecord(value) ||
    !Array.isArray(value.items) ||
    !Array.isArray(value.protocol_options)
  ) {
    throw new Error("invalid provider endpoint response");
  }
  const protocolOptions = value.protocol_options.map(parseProtocolOptions);
  const items = value.items.map(parseProviderEndpoint);
  for (const endpoint of items) {
    const option = protocolOptions.find(
      (candidate) =>
        candidate.providerKind === endpoint.providerKind &&
        candidate.acceptedProtocol === endpoint.protocolDialect,
    );
    const upstream = endpoint.upstreamProtocolDialect ?? endpoint.protocolDialect;
    if (!option?.upstreamProtocols.includes(upstream)) {
      throw new Error("invalid provider endpoint response");
    }
  }
  return {
    configRevision: readPositiveInteger(value.config_revision),
    items,
    protocolOptions,
  };
}

function parseProviderEndpoint(value: unknown): ProviderEndpoint {
  if (!isRecord(value)) {
    throw new Error("invalid provider endpoint response");
  }
  const providerKind = readProviderKind(value.provider_kind);
  const protocolDialect = readProtocolDialect(value.protocol_dialect);
  const upstreamProtocolDialect = readOptionalProtocolDialect(
    value.upstream_protocol_dialect,
  );
  if (upstreamProtocolDialect === protocolDialect) {
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
    upstreamProtocolDialect,
    enabled: readBoolean(value.enabled),
    configVersion: readPositiveInteger(value.config_version),
  };
}

function parseProtocolOptions(value: unknown): ProviderProtocolOptions {
  if (!isRecord(value) || !Array.isArray(value.upstream_protocols)) {
    throw new Error("invalid provider endpoint response");
  }
  const upstreamProtocols = value.upstream_protocols.map(readProtocolDialect);
  if (new Set(upstreamProtocols).size !== upstreamProtocols.length) {
    throw new Error("invalid provider endpoint response");
  }
  return {
    providerKind: readProviderKind(value.provider_kind),
    acceptedProtocol: readProtocolDialect(value.accepted_protocol),
    upstreamProtocols,
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
  if (
    value !== "openai_responses" &&
    value !== "openai_chat_completions" &&
    value !== "codex_backend" &&
    value !== "anthropic_messages"
  ) {
    throw new Error("invalid provider endpoint response");
  }
  return value;
}

function readOptionalProtocolDialect(value: unknown): ProtocolDialect | null {
  return value === null ? null : readProtocolDialect(value);
}
