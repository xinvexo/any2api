import {
  parseUpstreamRequestUsage,
  type UpstreamRequestUsage,
} from "@/shared/api/upstream-request-usage";

export type OAuthProvider = "codex" | "claude" | "grok";

interface OAuthStartCommon {
  provider: OAuthProvider;
  sessionId: string;
  expiresInSeconds: number;
}

export interface OAuthAuthorizationCodeStartResult extends OAuthStartCommon {
  flow: "authorization_code";
  authorizationUrl: string;
  redirectUri: string;
}

export interface OAuthDeviceCodeStartResult extends OAuthStartCommon {
  flow: "device_code";
  userCode: string;
  verificationUri: string;
  verificationUriComplete: string | null;
  pollIntervalSeconds: number;
}

export type OAuthStartResult =
  | OAuthAuthorizationCodeStartResult
  | OAuthDeviceCodeStartResult;

export interface OAuthActivationResult {
  provider: OAuthProvider;
  accountId: string;
  label: string;
  requestsPerMinute: number | null;
  enabled: boolean;
  safeAccountEmail: string | null;
  expiresAt: number | null;
  selectedModelCount: number;
  configVersion: number;
  configRevision: number;
}

export interface OAuthAccount {
  id: string;
  providerKind: OAuthProvider;
  label: string;
  requestsPerMinute: number | null;
  enabled: boolean;
  safeAccountEmail: string | null;
  expiresAt: number | null;
  tokenVersion: number;
  accountGeneration: number;
  configVersion: number;
  selectedModelCount: number;
  /** Models currently selected for public routing. */
  models: string[];
  /** Plan/provider catalog this OAuth account may use. */
  availableModels: string[];
  /** Official Codex `chatgpt_plan_type` from the ID Token. */
  planType: string | null;
  usage: UpstreamRequestUsage;
}

export interface OAuthAccountConfiguration {
  configRevision: number;
  items: OAuthAccount[];
}

export interface OAuthAccountUpdateInput {
  expectedRevision: number;
  expectedConfigVersion: number;
  label: string;
  requestsPerMinute: number | null;
  enabled: boolean;
}

export interface OAuthImportedAccount {
  id: string;
  providerKind: OAuthProvider;
  label: string;
  requestsPerMinute: number | null;
  enabled: boolean;
  safeAccountEmail: string | null;
  expiresAt: number | null;
  selectedModelCount: number;
  configVersion: number;
}

export interface OAuthImportResult {
  importedCount: number;
  configRevision: number;
  items: OAuthImportedAccount[];
}

export type OAuthDevicePollResult =
  | { status: "pending"; retryAfterSeconds: number }
  | { status: "complete"; account: OAuthActivationResult };

export function parseOAuthStartResult(value: unknown): OAuthStartResult {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const provider = readOAuthProvider(value.provider);
  const common = {
    provider,
    sessionId: readString(value.session_id),
    expiresInSeconds: readInteger(value.expires_in_seconds, 1),
  };
  if (value.flow === "authorization_code") {
    return {
      ...common,
      flow: "authorization_code",
      authorizationUrl: readHttpUrl(value.authorization_url),
      redirectUri: readHttpUrl(value.redirect_uri),
    };
  }
  if (value.flow === "device_code") {
    return {
      ...common,
      flow: "device_code",
      userCode: readString(value.user_code),
      verificationUri: readHttpUrl(value.verification_uri),
      verificationUriComplete: readOptionalHttpUrl(value.verification_uri_complete),
      pollIntervalSeconds: readInteger(value.poll_interval_seconds, 1),
    };
  }
  throw invalidResponse();
}

export function parseOAuthDevicePollResult(value: unknown): OAuthDevicePollResult {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  if (value.status === "pending") {
    return {
      status: "pending",
      retryAfterSeconds: readInteger(value.retry_after_seconds, 1),
    };
  }
  if (value.status === "complete") {
    return {
      status: "complete",
      account: parseOAuthActivationResult(value.account),
    };
  }
  throw invalidResponse();
}

export function parseOAuthActivationResult(value: unknown): OAuthActivationResult {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const provider = readOAuthProvider(value.provider);
  return {
    provider,
    accountId: readString(value.account_id),
    label: readString(value.label),
    requestsPerMinute: readOptionalRpm(value.requests_per_minute),
    enabled: readBoolean(value.enabled),
    safeAccountEmail: readOptionalString(value.safe_account_email),
    expiresAt: readOptionalInteger(value.expires_at, 0),
    selectedModelCount: readInteger(value.selected_model_count, 0),
    configVersion: readInteger(value.config_version, 1),
    configRevision: readInteger(value.config_revision, 1),
  };
}

export function parseOAuthAccountConfiguration(value: unknown): OAuthAccountConfiguration {
  if (!isRecord(value) || !Array.isArray(value.items)) {
    throw invalidResponse();
  }
  return {
    configRevision: readInteger(value.config_revision, 1),
    items: value.items.map(parseOAuthAccount),
  };
}

export function parseOAuthImportResult(value: unknown): OAuthImportResult {
  if (!isRecord(value) || !Array.isArray(value.items)) {
    throw invalidResponse();
  }
  const items = value.items.map(parseOAuthImportedAccount);
  const importedCount = readInteger(value.imported_count, 1);
  if (importedCount !== items.length) {
    throw invalidResponse();
  }
  return {
    importedCount,
    configRevision: readInteger(value.config_revision, 1),
    items,
  };
}

function parseOAuthImportedAccount(value: unknown): OAuthImportedAccount {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  return {
    id: readString(value.id),
    providerKind: readOAuthProvider(value.provider_kind),
    label: readString(value.label),
    requestsPerMinute: readOptionalRpm(value.requests_per_minute),
    enabled: readBoolean(value.enabled),
    safeAccountEmail: readOptionalString(value.safe_account_email),
    expiresAt: readOptionalInteger(value.expires_at, 0),
    selectedModelCount: readInteger(value.selected_model_count, 0),
    configVersion: readInteger(value.config_version, 1),
  };
}

function parseOAuthAccount(value: unknown): OAuthAccount {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const providerKind = readOAuthProvider(value.provider_kind);
  if (!Array.isArray(value.models) || !Array.isArray(value.available_models)) {
    throw invalidResponse();
  }
  const models = value.models.map(readString);
  const availableModels = value.available_models.map(readString);
  const selectedModelCount = readInteger(value.selected_model_count, 0);
  if (
    selectedModelCount !== models.length ||
    new Set(models).size !== models.length ||
    new Set(availableModels).size !== availableModels.length
  ) {
    throw invalidResponse();
  }
  return {
    id: readString(value.id),
    providerKind,
    label: readString(value.label),
    requestsPerMinute: readOptionalRpm(value.requests_per_minute),
    enabled: readBoolean(value.enabled),
    safeAccountEmail: readOptionalString(value.safe_account_email),
    expiresAt: readOptionalInteger(value.expires_at, 0),
    tokenVersion: readInteger(value.token_version, 1),
    accountGeneration: readInteger(value.account_generation, 1),
    configVersion: readInteger(value.config_version, 1),
    selectedModelCount,
    models,
    availableModels,
    planType: readOptionalString(value.plan_type),
    usage: parseUpstreamRequestUsage(value.usage),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown) {
  if (typeof value !== "string" || value.length === 0) {
    throw invalidResponse();
  }
  return value;
}

function readBoolean(value: unknown) {
  if (typeof value !== "boolean") {
    throw invalidResponse();
  }
  return value;
}

function readOAuthProvider(value: unknown): OAuthProvider {
  if (value !== "codex" && value !== "claude" && value !== "grok") {
    throw invalidResponse();
  }
  return value;
}

function readInteger(value: unknown, minimum: number) {
  if (
    typeof value !== "number" ||
    !Number.isInteger(value) ||
    value < minimum
  ) {
    throw invalidResponse();
  }
  return value;
}

function readOptionalInteger(value: unknown, minimum: number) {
  return value === null ? null : readInteger(value, minimum);
}

function readOptionalRpm(value: unknown) {
  const rpm = readOptionalInteger(value, 1);
  if (rpm !== null && rpm > 100_000) {
    throw invalidResponse();
  }
  return rpm;
}

function readOptionalString(value: unknown) {
  return value === null ? null : readString(value);
}

function readHttpUrl(value: unknown) {
  const text = readString(value);
  let url: URL;
  try {
    url = new URL(text);
  } catch {
    throw invalidResponse();
  }
  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw invalidResponse();
  }
  return text;
}

function readOptionalHttpUrl(value: unknown) {
  return value === null ? null : readHttpUrl(value);
}

function invalidResponse() {
  return new Error("invalid OAuth2 login response");
}
