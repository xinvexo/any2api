import { expect, test } from "vitest";

import { parseOverviewUsage } from "./overview-usage-contracts";
import { overviewUsageWire } from "./overview-usage-test-support";

test("parses exact decimal token totals beyond JavaScript safe integers", () => {
  const parsed = parseOverviewUsage(overviewUsageWire("1h"), "1h");

  expect(parsed.retained.totalTokens).toBe(9_007_199_254_741_000n);
  expect(parsed.timeBuckets).toHaveLength(12);
  expect(parsed.models.map((model) => model.publicModel)).toEqual(["gpt-test", null]);
});

test("rejects token, time bucket, and model conservation mismatches", () => {
  const tokenMismatch = structuredClone(overviewUsageWire());
  tokenMismatch.retained.total_tokens = "1";
  expect(() => parseOverviewUsage(tokenMismatch)).toThrow("invalid overview usage response");

  const missingBucket = structuredClone(overviewUsageWire());
  missingBucket.time_buckets.pop();
  expect(() => parseOverviewUsage(missingBucket)).toThrow("invalid overview usage response");

  const modelMismatch = structuredClone(overviewUsageWire());
  modelMismatch.models[0].request_count = 2;
  expect(() => parseOverviewUsage(modelMismatch)).toThrow("invalid overview usage response");
});

test("rejects an omitted current nullable retention boundary", () => {
  const omitted = structuredClone(overviewUsageWire()) as unknown as Record<string, unknown>;
  delete omitted.retained_started_at_ms;

  expect(() => parseOverviewUsage(omitted)).toThrow("invalid overview usage response");
});
