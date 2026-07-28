export interface OAuthQuotaWindow {
  id: string;
  kind: "time" | "credits";
  usedPercent: number;
  limitWindowSeconds: number | null;
  resetAfterSeconds: number | null;
  resetAt: number | null;
}

interface OAuthQuotaRateLimit {
  allowed: boolean | null;
  limitReached: boolean | null;
  windows: OAuthQuotaWindow[];
}

interface OAuthQuotaResetCredits {
  availableCount: number;
  expiresAt: string[];
}

interface OAuthQuotaBilling {
  currency: "USD";
  prepaidBalanceMinor: number | null;
  onDemandUsedMinor: number | null;
  onDemandCapMinor: number | null;
  isUnifiedBillingUser: boolean | null;
}

export interface OAuthQuotaTokenBalance {
  source: "upstream";
  used: number;
  limit: number;
  remaining: number;
  windowSeconds: number | null;
}

interface OAuthQuotaExhaustion {
  observedAt: number;
  used: number | null;
  limit: number | null;
}

export interface OAuthQuotaAccountStatus {
  authentication: "valid";
  userBlockedReason: string | null;
  teamBlockedReasons: string[];
  quotaExhaustion: OAuthQuotaExhaustion | null;
}

export interface OAuthQuotaSnapshot {
  fetchedAt: number;
  rateLimit: OAuthQuotaRateLimit | null;
  resetCredits: OAuthQuotaResetCredits | null;
  billing: OAuthQuotaBilling | null;
  tokenBalance: OAuthQuotaTokenBalance | null;
  subscriptionTier: string | null;
  accountStatus: OAuthQuotaAccountStatus | null;
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
    billing: parseBilling(value.billing),
    tokenBalance: parseTokenBalance(value.token_balance),
    subscriptionTier: readOptionalString(value.subscription_tier),
    accountStatus: parseAccountStatus(value.account_status),
  };
}

function parseTokenBalance(value: unknown): OAuthQuotaTokenBalance | null {
  if (value === null || value === undefined) return null;
  if (!isRecord(value)) throw invalidResponse();
  const source = value.source;
  if (source !== "upstream") throw invalidResponse();
  return {
    source,
    used: readInteger(value.used, 0),
    limit: readInteger(value.limit, 0),
    remaining: readInteger(value.remaining, 0),
    windowSeconds: readOptionalInteger(value.window_seconds, 1),
  };
}

function parseAccountStatus(value: unknown): OAuthQuotaAccountStatus | null {
  if (value === null || value === undefined) return null;
  if (
    !isRecord(value)
    || value.authentication !== "valid"
    || !Array.isArray(value.team_blocked_reasons)
  ) {
    throw invalidResponse();
  }
  return {
    authentication: "valid",
    userBlockedReason: readOptionalString(value.user_blocked_reason),
    teamBlockedReasons: value.team_blocked_reasons.map(readString),
    quotaExhaustion: parseQuotaExhaustion(value.quota_exhaustion),
  };
}

function parseQuotaExhaustion(value: unknown): OAuthQuotaExhaustion | null {
  if (value === null || value === undefined) return null;
  if (!isRecord(value)) throw invalidResponse();
  return {
    observedAt: readInteger(value.observed_at, 0),
    used: readOptionalInteger(value.used, 0),
    limit: readOptionalInteger(value.limit, 0),
  };
}

function parseBilling(value: unknown): OAuthQuotaBilling | null {
  if (value === null || value === undefined) return null;
  if (!isRecord(value) || value.currency !== "USD") throw invalidResponse();
  return {
    currency: "USD",
    prepaidBalanceMinor: readOptionalSafeInteger(value.prepaid_balance_minor),
    onDemandUsedMinor: readOptionalSafeInteger(value.on_demand_used_minor),
    onDemandCapMinor: readOptionalSafeInteger(value.on_demand_cap_minor),
    isUnifiedBillingUser: readOptionalBoolean(value.is_unified_billing_user),
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
  if (value === "time" || value === "credits") {
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

function readOptionalString(value: unknown) {
  return value === null || value === undefined ? null : readString(value);
}

function readBoolean(value: unknown) {
  if (typeof value !== "boolean") throw invalidResponse();
  return value;
}

function readOptionalBoolean(value: unknown) {
  return value === null || value === undefined ? null : readBoolean(value);
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

function readOptionalSafeInteger(value: unknown) {
  if (value === null || value === undefined) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value)) throw invalidResponse();
  return value;
}

function invalidResponse() {
  return new Error("invalid OAuth quota response");
}
