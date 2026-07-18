import type { ProtocolDialect } from "@/features/providers";

export interface RouteTarget {
  id: string;
  providerEndpointId: string;
  upstreamModel: string;
  fallbackTier: number;
  enabled: boolean;
}

export interface ModelRoute {
  id: string;
  publicModel: string;
  ingressProtocol: ProtocolDialect;
  fallbackOnSaturation: boolean | null;
  enabled: boolean;
  configVersion: number;
  targets: RouteTarget[];
}

export interface ModelRouteConfiguration {
  configRevision: number;
  items: ModelRoute[];
}

export interface RouteTargetWriteInput {
  id?: string;
  providerEndpointId: string;
  upstreamModel: string;
  fallbackTier: number;
  enabled: boolean;
}

export interface ModelRouteWriteInput {
  expectedRevision: number;
  expectedConfigVersion?: number;
  publicModel: string;
  ingressProtocol: ProtocolDialect;
  fallbackOnSaturation: boolean | null;
  enabled: boolean;
  targets: RouteTargetWriteInput[];
}

export function parseModelRouteConfiguration(value: unknown): ModelRouteConfiguration {
  if (!isRecord(value) || !Array.isArray(value.items)) {
    throw invalidResponse();
  }
  return {
    configRevision: readPositiveInteger(value.config_revision),
    items: value.items.map(parseModelRoute),
  };
}

function parseModelRoute(value: unknown): ModelRoute {
  if (!isRecord(value) || !Array.isArray(value.targets) || value.targets.length === 0) {
    throw invalidResponse();
  }
  const targets = value.targets.map(parseRouteTarget);
  const targetIds = new Set(targets.map((target) => target.id));
  if (targetIds.size !== targets.length) {
    throw invalidResponse();
  }
  return {
    id: readString(value.id),
    publicModel: readModelName(value.public_model),
    ingressProtocol: readProtocolDialect(value.ingress_protocol),
    fallbackOnSaturation: readOptionalBoolean(value.fallback_on_saturation),
    enabled: readBoolean(value.enabled),
    configVersion: readPositiveInteger(value.config_version),
    targets,
  };
}

function parseRouteTarget(value: unknown): RouteTarget {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  return {
    id: readString(value.id),
    providerEndpointId: readString(value.provider_endpoint_id),
    upstreamModel: readModelName(value.upstream_model),
    fallbackTier: readFallbackTier(value.fallback_tier),
    enabled: readBoolean(value.enabled),
  };
}

function readModelName(value: unknown): string {
  const model = readString(value);
  if (
    model.trim() !== model ||
    [...model].length > 255 ||
    [...model].some((character) => /\p{Cc}/u.test(character))
  ) {
    throw invalidResponse();
  }
  return model;
}

function readProtocolDialect(value: unknown): ProtocolDialect {
  if (value !== "openai_responses" && value !== "anthropic_messages") {
    throw invalidResponse();
  }
  return value;
}

function readFallbackTier(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) < 0 || Number(value) > 65_535) {
    throw invalidResponse();
  }
  return Number(value);
}

function readOptionalBoolean(value: unknown): boolean | null {
  if (value === null || typeof value === "boolean") {
    return value;
  }
  throw invalidResponse();
}

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw invalidResponse();
  }
  return value;
}

function readPositiveInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) <= 0) {
    throw invalidResponse();
  }
  return Number(value);
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) {
    throw invalidResponse();
  }
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function invalidResponse() {
  return new Error("invalid model route response");
}
