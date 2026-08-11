import type { OAuthQuotaWindow } from "./oauth-quota-contracts";

export type OAuthQuotaEstimateConfidence =
  | "unknown"
  | "learning"
  | "stable"
  | "degraded";

export type OAuthQuotaIntervalStatus =
  | "awaiting_baseline"
  | "no_change"
  | "valid_sample"
  | "reset_boundary"
  | "telemetry_incomplete"
  | "unpriced_usage"
  | "external_usage_suspected"
  | "outlier_rejected"
  | "invalid";

export interface OAuthQuotaIntervalDiagnostic {
  status: OAuthQuotaIntervalStatus;
  startedAt: number | null;
  endedAt: number;
  deltaUsedPercent: number | null;
  localCostCredits: number | null;
  unpricedRequestCount: number;
  queueDroppedRequestLogs: number;
  storageFailedRequestLogs: number;
  prunedRequestLogs: number;
}

export interface OAuthQuotaEstimate {
  windowId: string;
  windowKind: OAuthQuotaWindow["kind"];
  limitWindowSeconds: number | null;
  windowResetAt: number | null;
  epoch: number;
  epochStartedAt: number;
  confidence: OAuthQuotaEstimateConfidence;
  estimatedCapacityCredits: number | null;
  estimatedUsedCredits: number | null;
  estimatedRemainingCredits: number | null;
  sampleCount: number;
  relativeMad: number | null;
  latestInterval: OAuthQuotaIntervalDiagnostic;
  rateCards: string[];
}

export function parseOAuthQuotaEstimates(value: unknown): OAuthQuotaEstimate[] {
  if (!Array.isArray(value)) throw invalidResponse();
  return value.map(parseEstimate);
}

function parseEstimate(value: unknown): OAuthQuotaEstimate {
  if (!isRecord(value) || !Array.isArray(value.rate_cards)) throw invalidResponse();
  return {
    windowId: readString(value.window_id),
    windowKind: readWindowKind(value.window_kind),
    limitWindowSeconds: readOptionalInteger(value.limit_window_seconds),
    windowResetAt: readOptionalInteger(value.window_reset_at),
    epoch: readInteger(value.epoch, 1),
    epochStartedAt: readInteger(value.epoch_started_at),
    confidence: readConfidence(value.confidence),
    estimatedCapacityCredits: readOptionalNumber(value.estimated_capacity_credits),
    estimatedUsedCredits: readOptionalNumber(value.estimated_used_credits),
    estimatedRemainingCredits: readOptionalNumber(value.estimated_remaining_credits),
    sampleCount: readInteger(value.sample_count),
    relativeMad: readOptionalNumber(value.relative_mad),
    latestInterval: parseInterval(value.latest_interval),
    rateCards: value.rate_cards.map(readString),
  };
}

function parseInterval(value: unknown): OAuthQuotaIntervalDiagnostic {
  if (!isRecord(value)) throw invalidResponse();
  return {
    status: readIntervalStatus(value.status),
    startedAt: readOptionalInteger(value.started_at),
    endedAt: readInteger(value.ended_at),
    deltaUsedPercent: readOptionalFiniteNumber(value.delta_used_percent),
    localCostCredits: readOptionalNumber(value.local_cost_credits),
    unpricedRequestCount: readInteger(value.unpriced_request_count),
    queueDroppedRequestLogs: readInteger(value.queue_dropped_request_logs),
    storageFailedRequestLogs: readInteger(value.storage_failed_request_logs),
    prunedRequestLogs: readInteger(value.pruned_request_logs),
  };
}

function readConfidence(value: unknown): OAuthQuotaEstimateConfidence {
  if (value === "unknown" || value === "learning" || value === "stable" || value === "degraded") {
    return value;
  }
  throw invalidResponse();
}

function readIntervalStatus(value: unknown): OAuthQuotaIntervalStatus {
  const statuses: OAuthQuotaIntervalStatus[] = [
    "awaiting_baseline",
    "no_change",
    "valid_sample",
    "reset_boundary",
    "telemetry_incomplete",
    "unpriced_usage",
    "external_usage_suspected",
    "outlier_rejected",
    "invalid",
  ];
  if (typeof value === "string" && statuses.includes(value as OAuthQuotaIntervalStatus)) {
    return value as OAuthQuotaIntervalStatus;
  }
  throw invalidResponse();
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

function readOptionalNumber(value: unknown) {
  return value === null ? null : readNumber(value);
}

function readOptionalFiniteNumber(value: unknown) {
  if (value === null) return null;
  if (typeof value !== "number" || !Number.isFinite(value)) throw invalidResponse();
  return value;
}

function readInteger(value: unknown, minimum = 0) {
  const number = readNumber(value);
  if (!Number.isSafeInteger(number) || number < minimum) throw invalidResponse();
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
