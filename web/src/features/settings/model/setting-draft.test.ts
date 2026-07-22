import { expect, test } from "vitest";

import type { SettingItem } from "../api/settings-contracts";
import {
  createSettingDraft,
  isSettingDraftDirty,
  validateSettingDraft,
} from "./setting-draft";

test("keeps numeric input as text and validates empty, fractional, and bounded values", () => {
  const item = numericItem();
  expect(createSettingDraft(item)).toBe("128");
  expect(validateSettingDraft(item, "").error).toBe("请输入数值");
  expect(validateSettingDraft(item, "1.5").error).toBe("请输入非负整数");
  expect(validateSettingDraft(item, "201").error).toBe("不能大于 200");
  expect(validateSettingDraft(item, "64")).toEqual({ value: 64, error: null });
});

test("does not mark a draft dirty when it equals the effective value", () => {
  const item = numericItem();
  expect(isSettingDraftDirty(item, "128")).toBe(true);
  expect(isSettingDraftDirty(item, "64")).toBe(true);
  expect(isSettingDraftDirty({ ...item, overrideValue: 128 }, "128")).toBe(false);
});

test("edits duration settings in whole seconds", () => {
  const item = durationItem();
  expect(createSettingDraft(item)).toBe("30");
  expect(validateSettingDraft(item, "5")).toEqual({ value: 5, error: null });
  expect(validateSettingDraft(item, "0.5").error).toBe("请输入非负整数");
  expect(validateSettingDraft(item, "0").error).toMatch(/不能小于/);
  expect(validateSettingDraft(item, "90000").error).toMatch(/不能大于/);
  expect(isSettingDraftDirty({ ...item, overrideValue: 5 }, "5")).toBe(false);
  expect(isSettingDraftDirty({ ...item, overrideValue: 5 }, "6")).toBe(true);
});

function numericItem(): SettingItem {
  return {
    key: "scheduler.max_waiting_requests",
    valueType: "integer",
    defaultValue: 128,
    overrideValue: null,
    effectiveValue: 128,
    minValue: 1,
    maxValue: 200,
    allowedValues: null,
    applyMode: "hot_reload",
    webGroup: "排队策略",
    description: "Maximum queue size",
  };
}

function durationItem(): SettingItem {
  return {
    key: "scheduler.queue_timeout",
    valueType: "duration_secs",
    defaultValue: 30,
    overrideValue: null,
    effectiveValue: 30,
    minValue: 1,
    maxValue: 86_400,
    allowedValues: null,
    applyMode: "hot_reload",
    webGroup: "排队策略",
    description: "Queue timeout",
  };
}
