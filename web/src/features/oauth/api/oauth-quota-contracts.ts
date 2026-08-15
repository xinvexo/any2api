import {
  parseOAuthQuotaAccessStatus,
  parseOAuthQuotaCredits,
  type OAuthQuotaAccessStatus,
  type OAuthQuotaCredits,
} from "./oauth-quota-codex-contracts";
import {
  parseOAuthQuotaEstimates,
  type OAuthQuotaEstimate,
} from "./oauth-quota-estimate-contracts";

export type {
  OAuthQuotaAccessStatus,
  OAuthQuotaCredits,
  OAuthQuotaReachedType,
} from "./oauth-quota-codex-contracts";
export type {
  OAuthQuotaEstimate,
} from "./oauth-quota-estimate-contracts";

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

export interface OAuthQuotaRateCard {
  id: string;
  creditsPerUsd: number;
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
  credits: OAuthQuotaCredits | null;
  access: OAuthQuotaAccessStatus | null;
  resetCredits: OAuthQuotaResetCredits | null;
  billing: OAuthQuotaBilling | null;
  tokenBalance: OAuthQuotaTokenBalance | null;
  subscriptionTier: string | null;
  accountStatus: OAuthQuotaAccountStatus | null;
  estimates: OAuthQuotaEstimate[];
  rateCard: OAuthQuotaRateCard | null;
}

export interface OAuthQuotaResetResult {
  windowsReset: number;
}

export interface OAuthQuotaRefreshBatchResult {
  succeededAccountIds: string[];
  failedAccountIds: string[];
  modelCatalogRefreshedScopes: number;
  modelCatalogFailedScopes: number;
}

export function parseOAuthQuotaSnapshot(value: unknown): OAuthQuotaSnapshot {
  if (!isRecord(value)) throw invalidResponse();
  return {
    fetchedAt: readInteger(value.fetched_at, 0),
    rateLimit: parseRateLimit(value.rate_limit),
    credits: parseOAuthQuotaCredits(value.credits),
    access: parseOAuthQuotaAccessStatus(value.access),
    resetCredits: parseResetCredits(value.reset_credits),
    billing: parseBilling(value.billing),
    tokenBalance: parseTokenBalance(value.token_balance),
    subscriptionTier: readNullableString(value.subscription_tier),
    accountStatus: parseAccountStatus(value.account_status),
    estimates: parseOAuthQuotaEstimates(value.estimates),
    rateCard: parseRateCard(value.rate_card),
  };
}

function parseRateCard(value: unknown): OAuthQuotaRateCard | null {
  if (value === null) return null;
  if (!isRecord(value)) throw invalidResponse();
  return {
    id: readString(value.id),
    creditsPerUsd: readInteger(value.credits_per_usd, 1),
  };
}

export function parseNullableOAuthQuotaSnapshot(
  value: unknown,
): OAuthQuotaSnapshot | null {
  return value === null ? null : parseOAuthQuotaSnapshot(value);
}

function parseTokenBalance(value: unknown): OAuthQuotaTokenBalance | null {
  if (value === null) return null;
  if (!isRecord(value)) throw invalidResponse();
  const source = value.source;
  if (source !== "upstream") throw invalidResponse();
  return {
    source,
    used: readInteger(value.used, 0),
    limit: readInteger(value.limit, 0),
    remaining: readInteger(value.remaining, 0),
    windowSeconds: readNullableInteger(value.window_seconds, 1),
  };
}

function parseAccountStatus(value: unknown): OAuthQuotaAccountStatus | null {
  if (value === null) return null;
  if (
    !isRecord(value)
    || value.authentication !== "valid"
    || !Array.isArray(value.team_blocked_reasons)
  ) {
    throw invalidResponse();
  }
  return {
    authentication: "valid",
    userBlockedReason: readNullableString(value.user_blocked_reason),
    teamBlockedReasons: value.team_blocked_reasons.map(readString),
    quotaExhaustion: parseQuotaExhaustion(value.quota_exhaustion),
  };
}

function parseQuotaExhaustion(value: unknown): OAuthQuotaExhaustion | null {
  if (value === null) return null;
  if (!isRecord(value)) throw invalidResponse();
  return {
    observedAt: readInteger(value.observed_at, 0),
    used: readNullableInteger(value.used, 0),
    limit: readNullableInteger(value.limit, 0),
  };
}

function parseBilling(value: unknown): OAuthQuotaBilling | null {
  if (value === null) return null;
  if (!isRecord(value) || value.currency !== "USD") throw invalidResponse();
  return {
    currency: "USD",
    prepaidBalanceMinor: readNullableSafeInteger(value.prepaid_balance_minor),
    onDemandUsedMinor: readNullableSafeInteger(value.on_demand_used_minor),
    onDemandCapMinor: readNullableSafeInteger(value.on_demand_cap_minor),
    isUnifiedBillingUser: readNullableBoolean(value.is_unified_billing_user),
  };
}

export function parseOAuthQuotaResetResult(value: unknown): OAuthQuotaResetResult {
  if (!isRecord(value)) throw invalidResponse();
  return { windowsReset: readInteger(value.windows_reset, 1) };
}

export function parseOAuthQuotaRefreshBatchResult(
  value: unknown,
): OAuthQuotaRefreshBatchResult {
  if (
    !isRecord(value) ||
    !Array.isArray(value.succeeded_account_ids) ||
    !Array.isArray(value.failed_account_ids)
  ) {
    throw invalidResponse();
  }
  return {
    succeededAccountIds: value.succeeded_account_ids.map(readString),
    failedAccountIds: value.failed_account_ids.map(readString),
    modelCatalogRefreshedScopes: readInteger(
      value.model_catalog_refreshed_scopes,
      0,
    ),
    modelCatalogFailedScopes: readInteger(value.model_catalog_failed_scopes, 0),
  };
}

function parseRateLimit(value: unknown): OAuthQuotaRateLimit | null {
  if (value === null) return null;
  if (!isRecord(value) || !Array.isArray(value.windows)) throw invalidResponse();
  return {
    allowed: readNullableBoolean(value.allowed),
    limitReached: readNullableBoolean(value.limit_reached),
    windows: value.windows.map(parseWindow),
  };
}

function parseWindow(value: unknown): OAuthQuotaWindow {
  if (!isRecord(value)) throw invalidResponse();
  return {
    id: readString(value.id),
    kind: readWindowKind(value.kind),
    usedPercent: readNumber(value.used_percent, 0),
    limitWindowSeconds: readNullableInteger(value.limit_window_seconds, 0),
    resetAfterSeconds: readNullableInteger(value.reset_after_seconds, 0),
    resetAt: readNullableInteger(value.reset_at, 0),
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

function readNullableString(value: unknown) {
  return value === null ? null : readString(value);
}

function readBoolean(value: unknown) {
  if (typeof value !== "boolean") throw invalidResponse();
  return value;
}

function readNullableBoolean(value: unknown) {
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

function readNullableInteger(value: unknown, minimum: number) {
  return value === null ? null : readInteger(value, minimum);
}

function readNullableSafeInteger(value: unknown) {
  if (value === null) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value)) throw invalidResponse();
  return value;
}

function invalidResponse() {
  return new Error("invalid OAuth quota response");
}
