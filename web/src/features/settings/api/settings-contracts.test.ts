import { expect, test } from "vitest";

import { parseSettingsConfiguration } from "./settings-contracts";

test("parses setting metadata and all value types", () => {
  const configuration = parseSettingsConfiguration({
    config_revision: 2,
    items: [
      item("scheduler.on_rate_limited", "enum", "wait", "reject", ["wait", "reject"]),
      item("scheduler.queue_timeout", "duration_secs", 30, null, null, 1, 86_400),
      item("scheduler.max_waiting_requests", "integer", 128, null, null, 1, 100_000),
      item("scheduler.fallback_on_rate_limit", "boolean", false, null, null),
      item("models.allowed", "string_list", [], ["gpt-b"], null, null, null, ["gpt-a", "gpt-b"]),
      item("network.trusted_proxy_cidrs", "string_list", [], ["127.0.0.1/32"], null),
    ],
  });

  expect(configuration.configRevision).toBe(2);
  expect(configuration.items[1]?.valueType).toBe("duration_secs");
  expect(configuration.items[0]?.allowedValues).toEqual(["wait", "reject"]);
  expect(configuration.items[4]?.effectiveValue).toEqual(["gpt-b"]);
  expect(configuration.items[4]?.options).toEqual(["gpt-a", "gpt-b"]);
  expect(configuration.items[5]?.options).toBeNull();
  expect(configuration.items[5]?.effectiveValue).toEqual(["127.0.0.1/32"]);
});

test("accepts empty setting description", () => {
  const configuration = parseSettingsConfiguration({
    config_revision: 1,
    items: [{
      ...item("models.allowed", "string_list", [], null, null, null, null, ["gpt-a"]),
      description: "",
    }],
  });
  expect(configuration.items[0]?.description).toBe("");
});

test("rejects inconsistent bounds, values, and enum metadata", () => {
  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [item("scheduler.queue_timeout", "duration_secs", 30, null, null, 100, 10)],
  })).toThrow("invalid settings response");

  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [item("models.allowed", "string_list", null, null, null, null, null, [])],
  })).toThrow("invalid settings response");

  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [item("scheduler.on_rate_limited", "enum", "unknown", null, ["wait", "reject"])],
  })).toThrow("invalid settings response");

  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [{
      ...item("scheduler.fallback_on_rate_limit", "boolean", false, false, null),
      effective_value: true,
    }],
  })).toThrow("invalid settings response");
});

function item(
  key: string,
  valueType: string,
  defaultValue: boolean | number | string | string[] | null,
  overrideValue: boolean | number | string | string[] | null,
  allowedValues: string[] | null,
  minValue: number | null = null,
  maxValue: number | null = null,
  options: string[] | null = null,
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
    options,
    apply_mode: "hot_reload",
    web_group: "Test",
    description: "Test setting",
  };
}
