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
      item("models.allowed", "model_access", "all", ["gpt-b"], null, null, null, ["gpt-a", "gpt-b"]),
      item("network.trusted_proxy_cidrs", "string_list", [], ["127.0.0.1/32"], null),
      item(
        "oauth.codex.rate_card",
        "codex_rate_card",
        rateCard(),
        null,
        null,
        null,
        null,
        ["gpt-5.6-sol", "gpt-5.6-terra"],
      ),
    ],
  });

  expect(configuration.configRevision).toBe(2);
  expect(configuration.items[1]?.valueType).toBe("duration_secs");
  expect(configuration.items[0]?.allowedValues).toEqual(["wait", "reject"]);
  expect(configuration.items[4]?.effectiveValue).toEqual(["gpt-b"]);
  expect(configuration.items[4]?.options).toEqual(["gpt-a", "gpt-b"]);
  expect(configuration.items[5]?.options).toBeNull();
  expect(configuration.items[5]?.effectiveValue).toEqual(["127.0.0.1/32"]);
  expect(configuration.items[6]?.effectiveValue).toEqual(rateCard());
  expect(configuration.items[6]?.options).toEqual(["gpt-5.6-sol", "gpt-5.6-terra"]);
});

test("accepts empty setting description", () => {
  const configuration = parseSettingsConfiguration({
    config_revision: 1,
    items: [{
      ...item("models.allowed", "model_access", "all", null, null, null, null, ["gpt-a"]),
      description: "",
    }],
  });
  expect(configuration.items[0]?.description).toBe("");
});

test("parses restart-required connection limit metadata", () => {
  const configuration = parseSettingsConfiguration({
    config_revision: 1,
    items: [{
      ...item("network.max_connections", "integer", 4_096, 512, null, 1, 100_000),
      apply_mode: "restart_required",
      web_group: "入站网络",
    }],
  });

  expect(configuration.items[0]).toMatchObject({
    key: "network.max_connections",
    defaultValue: 4_096,
    overrideValue: 512,
    effectiveValue: 512,
    minValue: 1,
    maxValue: 100_000,
    applyMode: "restart_required",
    webGroup: "入站网络",
  });
});

test("rejects inconsistent bounds, values, and enum metadata", () => {
  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [item("scheduler.queue_timeout", "duration_secs", 30, null, null, 100, 10)],
  })).toThrow("invalid settings response");

  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [item("models.allowed", "model_access", null, null, null, null, null, [])],
  })).toThrow("invalid settings response");

  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [item("models.allowed", "model_access", "everything", null, null, null, null, [])],
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

test("rejects malformed Codex rate cards at the settings boundary", () => {
  const cachedAboveInput = rateCard();
  cachedAboveInput.models["gpt-5.6-sol"].standard.cached_input_nanos_per_million =
    cachedAboveInput.models["gpt-5.6-sol"].standard.input_nanos_per_million + 1;
  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [item("oauth.codex.rate_card", "codex_rate_card", cachedAboveInput, null, null)],
  })).toThrow("invalid settings response");

  expect(() => parseSettingsConfiguration({
    config_revision: 1,
    items: [item("oauth.codex.rate_card", "codex_rate_card", {
      ...rateCard(),
      obsolete_rate: 1,
    }, null, null)],
  })).toThrow("invalid settings response");
});

function item(
  key: string,
  valueType: string,
  defaultValue: boolean | number | string | string[] | Record<string, unknown> | null,
  overrideValue: boolean | number | string | string[] | Record<string, unknown> | null,
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

function rateCard() {
  return {
    id: "openai_codex_credits_2026_08_11",
    credits_per_usd: 25,
    models: {
      "gpt-5.6-sol": {
        standard: {
          input_nanos_per_million: 125_000_000_000,
          cached_input_nanos_per_million: 12_500_000_000,
          output_nanos_per_million: 750_000_000_000,
        },
      },
    },
  };
}
