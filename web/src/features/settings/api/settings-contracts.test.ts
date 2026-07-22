import { expect, test } from "vitest";

import { parseSettingsConfiguration } from "./settings-contracts";

test("parses setting metadata and all value types", () => {
  const configuration = parseSettingsConfiguration({
    config_revision: 2,
    items: [
      item("scheduler.on_saturated", "enum", "wait", "reject", ["wait", "reject"]),
      item("scheduler.queue_timeout", "duration_secs", 30, null, null, 1, 86_400),
      item("scheduler.max_waiting_requests", "integer", 128, null, null, 1, 100_000),
      item("scheduler.fallback_on_saturation", "boolean", false, null, null),
    ],
  });

  expect(configuration.configRevision).toBe(2);
  expect(configuration.items[1]?.valueType).toBe("duration_secs");
  expect(configuration.items[0]?.allowedValues).toEqual(["wait", "reject"]);
});

test("rejects inconsistent bounds, values, and enum metadata", () => {
  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [item("scheduler.queue_timeout", "duration_secs", 30, null, null, 100, 10)],
  })).toThrow("invalid settings response");

  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [item("scheduler.on_saturated", "enum", "unknown", null, ["wait", "reject"])],
  })).toThrow("invalid settings response");

  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [{
      ...item("scheduler.fallback_on_saturation", "boolean", false, false, null),
      effective_value: true,
    }],
  })).toThrow("invalid settings response");
});

function item(
  key: string,
  valueType: string,
  defaultValue: boolean | number | string,
  overrideValue: boolean | number | string | null,
  allowedValues: string[] | null,
  minValue: number | null = null,
  maxValue: number | null = null,
) {
  return {
    key,
    value_type: valueType,
    default_value: defaultValue,
    override_value: overrideValue,
    effective_value: overrideValue ?? defaultValue,
    min_value: minValue,
    max_value: maxValue,
    allowed_values: allowedValues,
    apply_mode: "hot_reload",
    web_group: "Test",
    description: "Test setting",
  };
}
