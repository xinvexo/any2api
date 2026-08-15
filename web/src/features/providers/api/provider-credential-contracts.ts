import {
  parseRequestUsage,
  type RequestUsage,
} from "@/shared/api/request-usage";
import {
  parseCredentialRuntime,
  type CredentialRuntime,
} from "@/shared/api/credential-runtime";

type CredentialKind = "api_key";
export const MAX_UPSTREAM_MODEL_NAME_CHARS = 255;

export interface CredentialModelSelection {
  upstreamModel: string;
  /** 公开别名；null 表示公开名称与上游名一致。 */
  publicModel: string | null;
}

export interface ProviderCredential {
  id: string;
  providerEndpointId: string;
  label: string;
  credentialKind: CredentialKind;
  fingerprint: string;
  secretTail: string | null;
  proxyProfileId: string;
  requestsPerMinute: number | null;
  enabled: boolean;
  secretVersion: number;
  credentialGeneration: number;
  configVersion: number;
  models: CredentialModelSelection[];
  runtime: CredentialRuntime;
  usage: RequestUsage;
}

export interface ProviderCredentialConfiguration {
  configRevision: number;
  providerEndpointId: string;
  items: ProviderCredential[];
}

export interface ProviderCredentialCreateInput {
  expectedRevision: number;
  label: string;
  apiKey: string;
  proxyProfileId: string;
  requestsPerMinute: number | null;
  enabled: boolean;
}

export interface ProviderCredentialUpdateInput {
  expectedRevision: number;
  expectedConfigVersion: number;
  label: string;
  proxyProfileId: string;
  requestsPerMinute: number | null;
  enabled: boolean;
}

export interface ProviderCredentialRotateInput {
  expectedRevision: number;
  expectedConfigVersion: number;
  expectedSecretVersion: number;
  apiKey: string;
}

export interface ProviderCredentialTestResult {
  configRevision: number;
  providerEndpointConfigVersion: number;
  credentialConfigVersion: number;
  credentialGeneration: number;
  secretVersion: number;
  proxyConfigVersion: number;
  credentialId: string;
  providerEndpointId: string;
  proxyId: string;
  reachable: boolean;
  accepted: boolean;
  catalogValid: boolean;
  statusCode: number | null;
  latencyMs: number;
  authErrorCleared: boolean;
  errorStage: string | null;
  failureScope: string | null;
  models: string[];
}

export interface ProviderCredentialModelsInput {
  expectedRevision: number;
  expectedConfigVersion: number;
  models: CredentialModelSelection[];
}

export function parseProviderCredentialConfiguration(
  value: unknown,
): ProviderCredentialConfiguration {
  if (!isRecord(value) || !Array.isArray(value.items)) {
    throw new Error("invalid provider credential response");
  }
  const providerEndpointId = readString(value.provider_endpoint_id);
  const items = value.items.map(parseProviderCredential);
  if (items.some((item) => item.providerEndpointId !== providerEndpointId)) {
    throw new Error("invalid provider credential response");
  }
  return {
    configRevision: readPositiveInteger(value.config_revision),
    providerEndpointId,
    items,
  };
}

export function parseProviderCredentialTestResult(value: unknown): ProviderCredentialTestResult {
  if (!isRecord(value)) {
    throw new Error("invalid provider credential test response");
  }
  const reachable = readBoolean(value.reachable);
  const accepted = readBoolean(value.accepted);
  const catalogValid = readBoolean(value.catalog_valid);
  const statusCode = readNullableStatusCode(value.status_code);
  const authErrorCleared = readBoolean(value.auth_error_cleared);
  const errorStage = readNullableString(value.error_stage);
  const failureScope = readNullableString(value.failure_scope);
  const models = readModelNames(value.models);
  if (
    reachable
      ? statusCode === null || errorStage !== null || failureScope !== null
      : accepted || statusCode !== null || errorStage === null || failureScope === null
  ) {
    throw new Error("invalid provider credential test response");
  }
  if (authErrorCleared && !accepted) {
    throw new Error("invalid provider credential test response");
  }
  if (catalogValid && !accepted || !catalogValid && models.length > 0) {
    throw new Error("invalid provider credential test response");
  }
  return {
    configRevision: readPositiveInteger(value.config_revision),
    providerEndpointConfigVersion: readPositiveInteger(value.provider_endpoint_config_version),
    credentialConfigVersion: readPositiveInteger(value.credential_config_version),
    credentialGeneration: readPositiveInteger(value.credential_generation),
    secretVersion: readPositiveInteger(value.secret_version),
    proxyConfigVersion: readPositiveInteger(value.proxy_config_version),
    credentialId: readString(value.credential_id),
    providerEndpointId: readString(value.provider_endpoint_id),
    proxyId: readString(value.proxy_id),
    reachable,
    accepted,
    catalogValid,
    statusCode,
    latencyMs: readNonNegativeInteger(value.latency_ms),
    authErrorCleared,
    errorStage,
    failureScope,
    models,
  };
}

function parseProviderCredential(value: unknown): ProviderCredential {
  if (
    !isRecord(value) ||
    value.credential_kind !== "api_key" ||
    "api_key" in value ||
    "secret" in value
  ) {
    throw new Error("invalid provider credential response");
  }
  const fingerprint = readString(value.fingerprint);
  if (!/^v2:[0-9a-f]{16}$/.test(fingerprint)) {
    throw new Error("invalid provider credential response");
  }
  const secretTail = readNullableString(value.secret_tail);
  if (secretTail !== null && (secretTail.length !== 4 || !isVisibleAscii(secretTail))) {
    throw new Error("invalid provider credential response");
  }
  return {
    id: readString(value.id),
    providerEndpointId: readString(value.provider_endpoint_id),
    label: readString(value.label),
    credentialKind: "api_key",
    fingerprint,
    secretTail,
    proxyProfileId: readString(value.proxy_profile_id),
    requestsPerMinute: readOptionalRpm(value.requests_per_minute),
    enabled: readBoolean(value.enabled),
    secretVersion: readPositiveInteger(value.secret_version),
    credentialGeneration: readPositiveInteger(value.credential_generation),
    configVersion: readPositiveInteger(value.config_version),
    models: readModelSelections(value.models),
    runtime: parseCredentialRuntime(value.runtime, "invalid provider credential response"),
    usage: parseRequestUsage(value.usage),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error("invalid provider credential response");
  }
  return value;
}

function readNullableString(value: unknown): string | null {
  return value === null ? null : readString(value);
}

function readPositiveInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) <= 0) {
    throw new Error("invalid provider credential response");
  }
  return Number(value);
}

function readNonNegativeInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) < 0) {
    throw new Error("invalid provider credential response");
  }
  return Number(value);
}

function readNullableStatusCode(value: unknown): number | null {
  if (value === null) {
    return null;
  }
  if (!Number.isSafeInteger(value) || Number(value) < 100 || Number(value) > 599) {
    throw new Error("invalid provider credential test response");
  }
  return Number(value);
}

function readOptionalRpm(value: unknown): number | null {
  if (value === null) {
    return null;
  }
  const parsed = readPositiveInteger(value);
  if (parsed > 100_000) {
    throw new Error("invalid provider credential response");
  }
  return parsed;
}

function readModelNames(value: unknown): string[] {
  if (!Array.isArray(value)) {
    throw new Error("invalid provider credential response");
  }
  const models = value.map((item) => readModelName(item));
  if (new Set(models).size !== models.length) {
    throw new Error("invalid provider credential response");
  }
  return models;
}

function readModelSelections(value: unknown): CredentialModelSelection[] {
  if (!Array.isArray(value)) {
    throw new Error("invalid provider credential response");
  }
  const models = value.map((item) => {
    if (!isRecord(item)) {
      throw new Error("invalid provider credential response");
    }
    const upstreamModel = readModelName(item.upstream_model);
    const publicModel = item.public_model === null || item.public_model === undefined
      ? null
      : readModelName(item.public_model);
    if (publicModel === upstreamModel) {
      throw new Error("invalid provider credential response");
    }
    return { upstreamModel, publicModel };
  });
  const upstreamNames = models.map((model) => model.upstreamModel);
  const publicNames = models.map((model) => model.publicModel ?? model.upstreamModel);
  if (
    new Set(upstreamNames).size !== models.length ||
    new Set(publicNames).size !== models.length
  ) {
    throw new Error("invalid provider credential response");
  }
  return models;
}

function readModelName(value: unknown): string {
  const model = readString(value);
  if (model.trim() !== model || [...model].length > MAX_UPSTREAM_MODEL_NAME_CHARS) {
    throw new Error("invalid provider credential response");
  }
  return model;
}

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new Error("invalid provider credential response");
  }
  return value;
}

function isVisibleAscii(value: string) {
  return [...value].every((character) => {
    const code = character.charCodeAt(0);
    return code >= 0x21 && code <= 0x7e;
  });
}
