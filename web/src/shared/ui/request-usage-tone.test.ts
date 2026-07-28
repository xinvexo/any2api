import { expect, test } from "vitest";

import {
  formatSuccessRate,
  requestUsageSlotTone,
  requestUsageSlotToneLabel,
  requestUsageSuccessRate,
} from "./request-usage-tone";

test("colors slots by success rate: green / yellow / red", () => {
  expect(requestUsageSlotTone(slot(0, 0, 0))).toBe("empty");
  expect(requestUsageSlotTone(slot(3, 3, 0))).toBe("ok");
  expect(requestUsageSlotTone(slot(2, 1, 1))).toBe("degraded");
  expect(requestUsageSlotTone(slot(4, 3, 1))).toBe("degraded");
  expect(requestUsageSlotTone(slot(3, 1, 2))).toBe("down");
  expect(requestUsageSlotTone(slot(2, 0, 2))).toBe("down");
});

test("labels match status-page semantics", () => {
  expect(requestUsageSlotToneLabel("empty")).toBe("无调用");
  expect(requestUsageSlotToneLabel("ok")).toBe("正常");
  expect(requestUsageSlotToneLabel("degraded")).toBe("降级");
  expect(requestUsageSlotToneLabel("down")).toBe("故障");
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
