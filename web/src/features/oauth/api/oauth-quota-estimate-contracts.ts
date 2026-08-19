import type { OAuthQuotaWindow } from "./oauth-quota-contracts";

export interface OAuthQuotaEstimate {
  windowId: string;
  windowKind: OAuthQuotaWindow["kind"];
  limitWindowSeconds: number | null;
  windowResetAt: number | null;
  estimatedCapacityCredits: number | null;
  estimatedUsedCredits: number | null;
  estimatedRemainingCredits: number | null;
}

export function parseOAuthQuotaEstimates(value: unknown): OAuthQuotaEstimate[] {
  if (!Array.isArray(value)) throw invalidResponse();
  return value.map(parseEstimate);
}

function parseEstimate(value: unknown): OAuthQuotaEstimate {
  if (!isRecord(value)) throw invalidResponse();
  return {
    windowId: readString(value.window_id),
    windowKind: readWindowKind(value.window_kind),
    limitWindowSeconds: readNullableInteger(value.limit_window_seconds),
    windowResetAt: readNullableInteger(value.window_reset_at),
    estimatedCapacityCredits: readNullableNumber(value.estimated_capacity_credits),
    estimatedUsedCredits: readNullableNumber(value.estimated_used_credits),
    estimatedRemainingCredits: readNullableNumber(value.estimated_remaining_credits),
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

function readNullableNumber(value: unknown) {
  return value === null ? null : readNumber(value);
}

function readInteger(value: unknown) {
  const number = readNumber(value);
  if (!Number.isSafeInteger(number)) throw invalidResponse();
  return number;
}

function readNullableInteger(value: unknown) {
  return value === null ? null : readInteger(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function invalidResponse() {
  return new Error("invalid OAuth quota response");
}
