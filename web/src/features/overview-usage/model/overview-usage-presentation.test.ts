import { expect, test } from "vitest";

import { calculateOverviewCacheHitRate } from "./overview-usage-presentation";

test("calculates a prompt cache token hit rate with one decimal precision", () => {
  expect(calculateOverviewCacheHitRate(4n, 10n)).toBe(40);
  expect(calculateOverviewCacheHitRate(2n, 3n)).toBe(66.7);
});

test("returns unknown without input tokens and bounds anomalous cache usage", () => {
  expect(calculateOverviewCacheHitRate(0n, 0n)).toBeNull();
  expect(calculateOverviewCacheHitRate(999n, 100n)).toBe(100);
});
