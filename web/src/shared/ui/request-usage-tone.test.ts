import { expect, test } from "vitest";

import {
  formatSuccessRate,
  requestUsageSlotBarClass,
  requestUsageSlotTone,
  requestUsageSuccessRate,
} from "./request-usage-tone";

test("colors slots by success rate: green / yellow / red", () => {
  expect(requestUsageSlotTone(slot(0, 0, 0))).toBe("empty");
  expect(requestUsageSlotTone(slot(100, 100, 0))).toBe("ok");
  expect(requestUsageSlotTone(slot(20, 19, 1))).toBe("ok");
  expect(requestUsageSlotTone(slot(17, 16, 1))).toBe("ok");
  expect(requestUsageSlotTone(slot(10, 9, 1))).toBe("ok");
  expect(requestUsageSlotTone(slot(1_000, 899, 101))).toBe("degraded");
  expect(requestUsageSlotTone(slot(5, 4, 1))).toBe("degraded");
  expect(requestUsageSlotTone(slot(2, 1, 1))).toBe("degraded");
  expect(requestUsageSlotTone(slot(2, 0, 2))).toBe("down");
});

test("uses application theme tokens for every slot fill", () => {
  expect(requestUsageSlotBarClass("empty")).toBe("bg-request-usage-empty");
  expect(requestUsageSlotBarClass("ok")).toBe("bg-request-usage-ok");
  expect(requestUsageSlotBarClass("degraded")).toBe("bg-request-usage-degraded");
  expect(requestUsageSlotBarClass("down")).toBe("bg-request-usage-down");
});

test("formats success rate for tooltips", () => {
  expect(requestUsageSuccessRate(slot(0, 0, 0))).toBeNull();
  expect(requestUsageSuccessRate(slot(4, 3, 1))).toBe(0.75);
  expect(formatSuccessRate(null)).toBe("—");
  expect(formatSuccessRate(0.5)).toBe("50%");
  expect(formatSuccessRate(1)).toBe("100%");
});

function slot(total: number, successful: number, failed: number) {
  return {
    startedAtMs: 0,
    totalRequests: total,
    successfulRequests: successful,
    failedRequests: failed,
  };
}
