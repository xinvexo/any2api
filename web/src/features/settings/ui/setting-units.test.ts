import { expect, test } from "vitest";

import type { SettingItem } from "../api/settings-contracts";
import {
  formatSettingUnitValue,
  parseSettingUnitValue,
  preferredSettingUnit,
  sanitizeSettingUnitInput,
  settingUnitKind,
  settingUnitOptions,
} from "./setting-units";

test("presents long durations in human units and converts them back to seconds", () => {
  const item = setting("logs.request.retention", "duration_secs");
  const month = preferredSettingUnit(item, 2_592_000);
  expect(month).toMatchObject({ value: "months", label: "月" });
  expect(formatSettingUnitValue("2592000", month)).toBe("1");
  expect(parseSettingUnitValue("1.5", settingUnitOptions("duration")[2])).toBe("5400");
  expect(preferredSettingUnit(item, 604_800).value).toBe("days");
  expect(preferredSettingUnit(item, 90).value).toBe("seconds");
});

test("presents byte budgets in binary KB/MB/GB units", () => {
  const item = setting("logs.file.max_total_size", "integer");
  expect(settingUnitKind(item)).toBe("bytes");
  const megabytes = preferredSettingUnit(item, 256 * 1024 * 1024);
  expect(megabytes).toMatchObject({ value: "mb", label: "MB" });
  expect(formatSettingUnitValue(String(256 * 1024 * 1024), megabytes)).toBe("256");
  expect(parseSettingUnitValue("1.5", settingUnitOptions("bytes")[2])).toBe(String(1.5 * 1024 ** 3));
});

test("filters invalid characters from scaled numeric inputs", () => {
  expect(sanitizeSettingUnitInput("1a.2.3MB")).toBe("1.23");
  expect(sanitizeSettingUnitInput("中文")).toBe("");
});

function setting(key: string, valueType: SettingItem["valueType"]): SettingItem {
  return {
    key,
    valueType,
    defaultValue: 1,
    overrideValue: null,
    effectiveValue: 1,
    minValue: 1,
    maxValue: 10_000_000_000,
    allowedValues: null,
    options: null,
    applyMode: "hot_reload",
    webGroup: "测试",
    description: "测试设置",
  };
}
