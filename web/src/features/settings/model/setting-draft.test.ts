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
  expect(isSettingDraftDirty(item, "128")).toBe(false);
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
  const overridden = { ...item, overrideValue: 5, effectiveValue: 5 };
  expect(isSettingDraftDirty(overridden, "5")).toBe(false);
  expect(isSettingDraftDirty(overridden, "6")).toBe(true);
});

test("keeps allow-all distinct from an empty exact model list", () => {
  const item = modelItem();
  expect(createSettingDraft(item)).toEqual({ mode: "only", models: ["gpt-b"] });
  expect(validateSettingDraft(item, {
    mode: "only",
    models: ["gpt-b", "gpt-a", "gpt-b"],
  })).toEqual({
    value: ["gpt-a", "gpt-b"],
    error: null,
  });
  expect(validateSettingDraft(item, { mode: "only", models: ["removed-model"] }).error)
    .toMatch(/列表已发生变化/);
  expect(validateSettingDraft(item, { mode: "all", models: [] })).toEqual({
    value: "all",
    error: null,
  });
  expect(validateSettingDraft(item, { mode: "only", models: [] })).toEqual({
    value: [],
    error: null,
  });
  expect(createSettingDraft({ ...item, overrideValue: null, effectiveValue: "all" })).toEqual({
    mode: "all",
    models: [],
  });
  expect(isSettingDraftDirty(item, { mode: "only", models: ["gpt-b"] })).toBe(false);
  expect(isSettingDraftDirty(item, { mode: "only", models: [] })).toBe(true);
});

test("edits free-form trusted proxy addresses one per line or comma", () => {
  const item = trustedProxyItem();
  expect(createSettingDraft(item)).toBe("10.0.0.0/8\n127.0.0.1/32");
  expect(validateSettingDraft(item, "10.0.0.0/8, 127.0.0.1\n10.0.0.0/8"))
    .toEqual({
      value: ["10.0.0.0/8", "127.0.0.1"],
      error: null,
    });
  expect(validateSettingDraft(item, "  \n")).toEqual({ value: [], error: null });
  expect(isSettingDraftDirty(item, "10.0.0.0/8\n127.0.0.1/32")).toBe(false);
});

test("edits a structured Codex rate card and requires a new ID for changed rates", () => {
  const item = rateCardItem();
  const draft = createSettingDraft(item);
  expect(typeof draft).toBe("string");
  expect(validateSettingDraft(item, draft).error).toBeNull();

  const changed = JSON.parse(String(draft));
  changed.credits_per_usd = 30;
  expect(validateSettingDraft(item, JSON.stringify(changed)).error).toMatch(/更换卡片 ID/);
  changed.id = "openai_codex_credits_2026_08_13";
  expect(validateSettingDraft(item, JSON.stringify(changed)).error).toBeNull();
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
    options: null,
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
    options: null,
    applyMode: "hot_reload",
    webGroup: "排队策略",
    description: "Queue timeout",
  };
}

function modelItem(): SettingItem {
  return {
    key: "models.allowed",
    valueType: "model_access",
    defaultValue: "all",
    overrideValue: ["gpt-b"],
    effectiveValue: ["gpt-b"],
    minValue: null,
    maxValue: null,
    allowedValues: null,
    options: ["gpt-a", "gpt-b"],
    applyMode: "hot_reload",
    webGroup: "公开模型",
    description: "Allowed public models",
  };
}

function trustedProxyItem(): SettingItem {
  return {
    key: "network.trusted_proxy_cidrs",
    valueType: "string_list",
    defaultValue: [],
    overrideValue: ["10.0.0.0/8", "127.0.0.1/32"],
    effectiveValue: ["10.0.0.0/8", "127.0.0.1/32"],
    minValue: null,
    maxValue: null,
    allowedValues: null,
    options: null,
    applyMode: "hot_reload",
    webGroup: "远程管理",
    description: "Trusted reverse proxies",
  };
}

function rateCardItem(): SettingItem {
  const value = {
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
  return {
    key: "oauth.codex.rate_card",
    valueType: "codex_rate_card",
    defaultValue: value,
    overrideValue: null,
    effectiveValue: value,
    minValue: null,
    maxValue: null,
    allowedValues: null,
    options: null,
    applyMode: "hot_reload",
    webGroup: "Codex 额度估算",
    description: "Codex rate card",
  };
}
