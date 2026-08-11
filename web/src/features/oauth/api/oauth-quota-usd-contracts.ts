import type { OAuthQuotaWindow } from "./oauth-quota-contracts";

export interface OAuthQuotaUsdEstimate {
  windowId: string;
  windowKind: OAuthQuotaWindow["kind"];
  limitWindowSeconds: number | null;
  windowResetAt: number | null;
  estimatedCapacityUsd: number;
  estimatedUsedUsd: number;
  estimatedRemainingUsd: number;
  sampleCostUsd: number;
  sampleUsedPercent: number;
  sampleStartedAt: number;
  sampleEndedAt: number;
  unpricedRequestCount: number;
  pricingBasis: string;
}

export function parseOAuthQuotaUsdEstimates(value: unknown): OAuthQuotaUsdEstimate[] {
  if (!Array.isArray(value)) throw invalidResponse();
  return value.map(parseEstimate);
}

function parseEstimate(value: unknown): OAuthQuotaUsdEstimate {
  if (!isRecord(value)) throw invalidResponse();
  return {
    windowId: readString(value.window_id),
    windowKind: readWindowKind(value.window_kind),
    limitWindowSeconds: readOptionalInteger(value.limit_window_seconds),
    windowResetAt: readOptionalInteger(value.window_reset_at),
    estimatedCapacityUsd: readPositiveNumber(value.estimated_capacity_usd),
    estimatedUsedUsd: readNumber(value.estimated_used_usd),
    estimatedRemainingUsd: readNumber(value.estimated_remaining_usd),
    sampleCostUsd: readPositiveNumber(value.sample_cost_usd),
    sampleUsedPercent: readPositiveNumber(value.sample_used_percent),
    sampleStartedAt: readInteger(value.sample_started_at),
    sampleEndedAt: readInteger(value.sample_ended_at),
    unpricedRequestCount: readInteger(value.unpriced_request_count),
    pricingBasis: readString(value.pricing_basis),
  };
}

function readWindowKind(value: unknown): OAuthQuotaWindow["kind"] {
  if (value === "time" || value === "credits") return value;
  throw invalidResponse();
}

function readString(value: unknown) {
  if (typeof value !== "string" || value.trim().length === 0) throw invalidResponse();
  return value;
}

function readNumber(value: unknown) {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
    throw invalidResponse();
  }
  return value;
}

function readPositiveNumber(value: unknown) {
  const number = readNumber(value);
  if (number === 0) throw invalidResponse();
  return number;
}

function readInteger(value: unknown) {
  const number = readNumber(value);
  if (!Number.isSafeInteger(number)) throw invalidResponse();
  return number;
}

function readOptionalInteger(value: unknown) {
  return value === null ? null : readInteger(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function invalidResponse() {
  return new Error("invalid OAuth quota response");
}
