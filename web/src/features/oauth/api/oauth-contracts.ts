export type OAuthProvider = "codex" | "claude";

export interface OAuthStartResult {
  provider: OAuthProvider;
  sessionId: string;
  authorizationUrl: string;
  redirectUri: string;
  expiresInSeconds: number;
}

export interface OAuthActivationResult {
  provider: OAuthProvider;
  accountId: string;
  label: string;
  maxConcurrency: number;
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
  maxConcurrency: number;
  enabled: boolean;
  safeAccountEmail: string | null;
  expiresAt: number | null;
  tokenVersion: number;
  accountGeneration: number;
  configVersion: number;
  selectedModelCount: number;
  models: string[];
}

export interface OAuthAccountConfiguration {
  configRevision: number;
  items: OAuthAccount[];
}

export interface OAuthAccountUpdateInput {
  expectedRevision: number;
  expectedConfigVersion: number;
  label: string;
  maxConcurrency: number;
  enabled: boolean;
}

export interface OAuthAccountModelsInput {
  expectedRevision: number;
  expectedConfigVersion: number;
  models: string[];
}

export function parseOAuthStartResult(value: unknown): OAuthStartResult {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const provider = value.provider;
  if (provider !== "codex" && provider !== "claude") {
    throw invalidResponse();
  }
  const expiresInSeconds = value.expires_in_seconds;
  if (
    typeof expiresInSeconds !== "number" ||
    !Number.isInteger(expiresInSeconds) ||
    expiresInSeconds <= 0
  ) {
    throw invalidResponse();
  }
  return {
    provider,
    sessionId: readString(value.session_id),
    authorizationUrl: readHttpUrl(value.authorization_url),
    redirectUri: readHttpUrl(value.redirect_uri),
    expiresInSeconds,
  };
}

export function parseOAuthActivationResult(value: unknown): OAuthActivationResult {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const provider = value.provider;
  if (provider !== "codex" && provider !== "claude") {
    throw invalidResponse();
  }
  return {
    provider,
    accountId: readString(value.account_id),
    label: readString(value.label),
    maxConcurrency: readInteger(value.max_concurrency, 1),
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

function parseOAuthAccount(value: unknown): OAuthAccount {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const providerKind = value.provider_kind;
  if (providerKind !== "codex" && providerKind !== "claude") {
    throw invalidResponse();
  }
  if (!Array.isArray(value.models)) {
    throw invalidResponse();
  }
  const models = value.models.map(readString);
  const selectedModelCount = readInteger(value.selected_model_count, 0);
  if (selectedModelCount !== models.length || new Set(models).size !== models.length) {
    throw invalidResponse();
  }
  return {
    id: readString(value.id),
    providerKind,
    label: readString(value.label),
    maxConcurrency: readInteger(value.max_concurrency, 1),
    enabled: readBoolean(value.enabled),
    safeAccountEmail: readOptionalString(value.safe_account_email),
    expiresAt: readOptionalInteger(value.expires_at, 0),
    tokenVersion: readInteger(value.token_version, 1),
    accountGeneration: readInteger(value.account_generation, 1),
    configVersion: readInteger(value.config_version, 1),
    selectedModelCount,
    models,
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

function invalidResponse() {
  return new Error("invalid OAuth2 login response");
}
