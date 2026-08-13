const OVERVIEW_USAGE_RANGES = ["1h", "24h", "7d", "30d"] as const;
export type OverviewUsageRange = (typeof OVERVIEW_USAGE_RANGES)[number];

export const OVERVIEW_USAGE_RANGE_SPECS: Record<
  OverviewUsageRange,
  { bucketCount: number; bucketWidthMs: number }
> = {
  "1h": { bucketCount: 12, bucketWidthMs: 5 * 60_000 },
  "24h": { bucketCount: 24, bucketWidthMs: 60 * 60_000 },
  "7d": { bucketCount: 28, bucketWidthMs: 6 * 60 * 60_000 },
  "30d": { bucketCount: 30, bucketWidthMs: 24 * 60 * 60_000 },
};

interface OverviewUsageTotals {
  requestCount: number;
  successfulRequestCount: number;
  failedRequestCount: number;
  tokenUsageRequestCount: number;
  inputTokens: bigint;
  outputTokens: bigint;
  totalTokens: bigint;
}

export interface OverviewUsageTimeBucket {
  startedAtMs: number;
  endedAtMs: number;
  requestCount: number;
  successfulRequestCount: number;
  failedRequestCount: number;
}

export interface OverviewUsageModel extends OverviewUsageTotals {
  publicModel: string | null;
  isOther: boolean;
}

export interface OverviewUsage {
  generatedAtMs: number;
  range: OverviewUsageRange;
  rangeStartedAtMs: number;
  rangeEndedAtMs: number;
  retainedStartedAtMs: number | null;
  retained: OverviewUsageTotals;
  selected: OverviewUsageTotals;
  timeBuckets: OverviewUsageTimeBucket[];
  models: OverviewUsageModel[];
}

export function isOverviewUsageRange(value: string | null): value is OverviewUsageRange {
  return OVERVIEW_USAGE_RANGES.some((range) => range === value);
}

export function parseOverviewUsage(
  value: unknown,
  expectedRange?: OverviewUsageRange,
): OverviewUsage {
  if (!isRecord(value)) throw invalidResponse();
  const wire = value;
  const rangeValue = typeof wire.range === "string" ? wire.range : null;
  if (!isOverviewUsageRange(rangeValue)) {
    throw invalidResponse();
  }
  if (expectedRange && rangeValue !== expectedRange) throw invalidResponse();
  const range = rangeValue;
  const generatedAtMs = readCount(wire.generated_at_ms);
  const rangeStartedAtMs = readCount(wire.range_started_at_ms);
  const rangeEndedAtMs = readCount(wire.range_ended_at_ms);
  const retainedStartedAtMs = readNullableCount(wire.retained_started_at_ms);
  const retained = parseTotals(wire.retained);
  const selected = parseTotals(wire.selected);
  const timeBuckets = parseTimeBuckets(
    wire.time_buckets,
    range,
    rangeStartedAtMs,
    rangeEndedAtMs,
    selected,
  );
  const models = parseModels(wire.models, selected);
  if (
    rangeEndedAtMs <= rangeStartedAtMs ||
    rangeEndedAtMs - rangeStartedAtMs !==
      OVERVIEW_USAGE_RANGE_SPECS[range].bucketCount *
        OVERVIEW_USAGE_RANGE_SPECS[range].bucketWidthMs ||
    (retainedStartedAtMs !== null && retainedStartedAtMs > generatedAtMs)
  ) {
    throw invalidResponse();
  }
  return {
    generatedAtMs,
    range,
    rangeStartedAtMs,
    rangeEndedAtMs,
    retainedStartedAtMs,
    retained,
    selected,
    timeBuckets,
    models,
  };
}

function parseTotals(value: unknown): OverviewUsageTotals {
  if (!isRecord(value)) throw invalidResponse();
  const wire = value;
  const requestCount = readCount(wire.request_count);
  const successfulRequestCount = readCount(wire.successful_request_count);
  const failedRequestCount = readCount(wire.failed_request_count);
  const tokenUsageRequestCount = readCount(wire.token_usage_request_count);
  const inputTokens = readTokenCount(wire.input_tokens);
  const outputTokens = readTokenCount(wire.output_tokens);
  const totalTokens = readTokenCount(wire.total_tokens);
  if (
    successfulRequestCount + failedRequestCount !== requestCount ||
    tokenUsageRequestCount > requestCount ||
    inputTokens + outputTokens !== totalTokens
  ) {
    throw invalidResponse();
  }
  return {
    requestCount,
    successfulRequestCount,
    failedRequestCount,
    tokenUsageRequestCount,
    inputTokens,
    outputTokens,
    totalTokens,
  };
}

function parseTimeBuckets(
  value: unknown,
  range: OverviewUsageRange,
  rangeStartedAtMs: number,
  rangeEndedAtMs: number,
  selected: OverviewUsageTotals,
) {
  if (!Array.isArray(value)) throw invalidResponse();
  const spec = OVERVIEW_USAGE_RANGE_SPECS[range];
  if (value.length !== spec.bucketCount) throw invalidResponse();
  const buckets = value.map((entry, index) => {
    if (!isRecord(entry)) throw invalidResponse();
    const wire = entry;
    const bucket = {
      startedAtMs: readCount(wire.started_at_ms),
      endedAtMs: readCount(wire.ended_at_ms),
      requestCount: readCount(wire.request_count),
      successfulRequestCount: readCount(wire.successful_request_count),
      failedRequestCount: readCount(wire.failed_request_count),
    };
    if (
      bucket.startedAtMs !== rangeStartedAtMs + index * spec.bucketWidthMs ||
      bucket.endedAtMs !== bucket.startedAtMs + spec.bucketWidthMs ||
      bucket.successfulRequestCount + bucket.failedRequestCount !== bucket.requestCount
    ) {
      throw invalidResponse();
    }
    return bucket;
  });
  if (buckets.at(-1)?.endedAtMs !== rangeEndedAtMs) throw invalidResponse();
  assertNumberSums(
    buckets.map((bucket) => bucket.requestCount),
    selected.requestCount,
  );
  assertNumberSums(
    buckets.map((bucket) => bucket.successfulRequestCount),
    selected.successfulRequestCount,
  );
  return buckets;
}

function parseModels(value: unknown, selected: OverviewUsageTotals) {
  if (!Array.isArray(value) || value.length > 13) throw invalidResponse();
  const seen = new Set<string>();
  const models = value.map((entry, index) => {
    if (!isRecord(entry)) throw invalidResponse();
    const wire = entry;
    const publicModel = readNullableString(wire.public_model);
    if (typeof wire.is_other !== "boolean") throw invalidResponse();
    if (wire.is_other && (publicModel !== null || index !== value.length - 1)) {
      throw invalidResponse();
    }
    const key = wire.is_other ? "other" : publicModel === null ? "unknown" : `model:${publicModel}`;
    if (seen.has(key)) throw invalidResponse();
    seen.add(key);
    return {
      publicModel,
      isOther: wire.is_other,
      ...parseTotals(entry),
    };
  });
  assertNumberSums(models.map((model) => model.requestCount), selected.requestCount);
  assertNumberSums(
    models.map((model) => model.successfulRequestCount),
    selected.successfulRequestCount,
  );
  assertNumberSums(
    models.map((model) => model.tokenUsageRequestCount),
    selected.tokenUsageRequestCount,
  );
  assertBigIntSums(models.map((model) => model.inputTokens), selected.inputTokens);
  assertBigIntSums(models.map((model) => model.outputTokens), selected.outputTokens);
  return models;
}

function assertNumberSums(values: number[], expected: number) {
  if (values.reduce((total, value) => total + value, 0) !== expected) throw invalidResponse();
}

function assertBigIntSums(values: bigint[], expected: bigint) {
  if (values.reduce((total, value) => total + value, 0n) !== expected) throw invalidResponse();
}

function readCount(value: unknown) {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw invalidResponse();
  }
  return value;
}

function readNullableCount(value: unknown) {
  return value === null ? null : readCount(value);
}

function readTokenCount(value: unknown) {
  if (typeof value !== "string" || !/^(0|[1-9]\d*)$/.test(value)) throw invalidResponse();
  return BigInt(value);
}

function readNullableString(value: unknown) {
  if (value === null) return null;
  if (typeof value !== "string" || value.length === 0) throw invalidResponse();
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function invalidResponse() {
  return new Error("invalid overview usage response");
}
