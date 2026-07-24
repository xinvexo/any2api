export interface OAuthQuotaWindow {
  usedPercent: number;
  limitWindowSeconds: number;
  resetAfterSeconds: number;
  resetAt: number;
}

export interface OAuthQuotaRateLimit {
  allowed: boolean;
  limitReached: boolean;
  primaryWindow: OAuthQuotaWindow | null;
  secondaryWindow: OAuthQuotaWindow | null;
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
  if (!isRecord(value)) throw invalidResponse();
  return {
    allowed: readBoolean(value.allowed),
    limitReached: readBoolean(value.limit_reached),
    primaryWindow: parseWindow(value.primary_window),
    secondaryWindow: parseWindow(value.secondary_window),
  };
}

function parseWindow(value: unknown): OAuthQuotaWindow | null {
  if (value === null) return null;
  if (!isRecord(value)) throw invalidResponse();
  return {
    usedPercent: readNumber(value.used_percent, 0),
    limitWindowSeconds: readInteger(value.limit_window_seconds, 0),
    resetAfterSeconds: readInteger(value.reset_after_seconds, 0),
    resetAt: readInteger(value.reset_at, 0),
  };
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

function invalidResponse() {
  return new Error("invalid OAuth quota response");
}
