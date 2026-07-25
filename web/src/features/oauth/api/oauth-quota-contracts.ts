export interface OAuthQuotaWindow {
  id: string;
  kind: "time" | "credits" | "requests" | "tokens";
  usedPercent: number;
  limitWindowSeconds: number | null;
  resetAfterSeconds: number | null;
  resetAt: number | null;
}

export interface OAuthQuotaRateLimit {
  allowed: boolean | null;
  limitReached: boolean | null;
  windows: OAuthQuotaWindow[];
}

export interface OAuthQuotaResetCredits {
  availableCount: number;
  expiresAt: string[];
}

export interface OAuthQuotaSnapshot {
  fetchedAt: number;
  rateLimit: OAuthQuotaRateLimit | null;
  resetCredits: OAuthQuotaResetCredits | null;
}

export interface OAuthQuotaResetResult {
  windowsReset: number;
}

export function parseOAuthQuotaSnapshot(value: unknown): OAuthQuotaSnapshot {
  if (!isRecord(value)) throw invalidResponse();
  return {
    fetchedAt: readInteger(value.fetched_at, 0),
    rateLimit: parseRateLimit(value.rate_limit),
    resetCredits: parseResetCredits(value.reset_credits),
  };
}

export function parseOAuthQuotaResetResult(value: unknown): OAuthQuotaResetResult {
  if (!isRecord(value)) throw invalidResponse();
  return { windowsReset: readInteger(value.windows_reset, 1) };
}

function parseRateLimit(value: unknown): OAuthQuotaRateLimit | null {
  if (value === null) return null;
  if (!isRecord(value) || !Array.isArray(value.windows)) throw invalidResponse();
  return {
    allowed: readOptionalBoolean(value.allowed),
    limitReached: readOptionalBoolean(value.limit_reached),
    windows: value.windows.map(parseWindow),
  };
}

function parseWindow(value: unknown): OAuthQuotaWindow {
  if (!isRecord(value)) throw invalidResponse();
  return {
    id: readString(value.id),
    kind: readWindowKind(value.kind),
    usedPercent: readNumber(value.used_percent, 0),
    limitWindowSeconds: readOptionalInteger(value.limit_window_seconds, 0),
    resetAfterSeconds: readOptionalInteger(value.reset_after_seconds, 0),
    resetAt: readOptionalInteger(value.reset_at, 0),
  };
}

function readWindowKind(value: unknown): OAuthQuotaWindow["kind"] {
  if (value === "time" || value === "credits" || value === "requests" || value === "tokens") {
    return value;
  }
  throw invalidResponse();
}

function parseResetCredits(value: unknown): OAuthQuotaResetCredits | null {
  if (value === null) return null;
  if (!isRecord(value) || !Array.isArray(value.expires_at)) {
    throw invalidResponse();
  }
  return {
    availableCount: readInteger(value.available_count, 0),
    expiresAt: value.expires_at.map(readString),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw invalidResponse();
  }
  return value;
}

function readBoolean(value: unknown) {
  if (typeof value !== "boolean") throw invalidResponse();
  return value;
}

function readOptionalBoolean(value: unknown) {
  return value === null ? null : readBoolean(value);
}

function readNumber(value: unknown, minimum: number) {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum) {
    throw invalidResponse();
  }
  return value;
}

function readInteger(value: unknown, minimum: number) {
  const number = readNumber(value, minimum);
  if (!Number.isSafeInteger(number)) throw invalidResponse();
  return number;
}

function readOptionalInteger(value: unknown, minimum: number) {
  return value === null ? null : readInteger(value, minimum);
}

function invalidResponse() {
  return new Error("invalid OAuth quota response");
}
