import type { SettingItem, SettingValue } from "../api/settings-contracts";

export interface ModelAccessDraft {
  mode: "all" | "only";
  models: string[];
}

export type SettingDraft = boolean | string | ModelAccessDraft;

export interface SettingDraftValidation {
  value: SettingValue | undefined;
  error: string | null;
}

export function createSettingDraft(item: SettingItem): SettingDraft {
  if (item.valueType === "optional_string_list") {
    if (item.effectiveValue === null) {
      return { mode: "all", models: [] };
    }
    if (Array.isArray(item.effectiveValue)) {
      return { mode: "only", models: [...item.effectiveValue] };
    }
    throw new Error("invalid model access setting");
  }
  if (typeof item.effectiveValue === "number") {
    return String(item.effectiveValue);
  }
  if (typeof item.effectiveValue === "boolean" || typeof item.effectiveValue === "string") {
    return item.effectiveValue;
  }
  throw new Error("invalid setting value");
}

export function validateSettingDraft(
  item: SettingItem,
  draft: SettingDraft,
): SettingDraftValidation {
  if (item.valueType === "optional_string_list") {
    if (!isModelAccessDraft(draft)) {
      return invalid("模型选择格式不正确");
    }
    if (draft.mode === "all") {
      return { value: null, error: null };
    }
    const values = [...new Set(draft.models)].sort();
    if (values.some((value) => !item.options?.includes(value))) {
      return invalid("模型列表已发生变化，请刷新后重试");
    }
    return { value: values, error: null };
  }
  if (item.valueType === "boolean") {
    return typeof draft === "boolean"
      ? { value: draft, error: null }
      : invalid("请选择启用或关闭");
  }
  if (typeof draft !== "string") {
    return invalid("设置值格式不正确");
  }
  if (item.valueType === "enum") {
    return item.allowedValues?.includes(draft)
      ? { value: draft, error: null }
      : invalid("请选择有效选项");
  }

  const text = draft.trim();
  if (text.length === 0) {
    return invalid("请输入数值");
  }
  if (!/^\d+$/.test(text)) {
    return invalid("请输入非负整数");
  }
  const value = Number(text);
  if (!Number.isSafeInteger(value)) {
    return invalid("数值过大");
  }
  if (item.minValue !== null && value < item.minValue) {
    return invalid(`不能小于 ${item.minValue}`);
  }
  if (item.maxValue !== null && value > item.maxValue) {
    return invalid(`不能大于 ${item.maxValue}`);
  }
  return { value, error: null };
}

export function isSettingDraftDirty(item: SettingItem, draft: SettingDraft) {
  const validation = validateSettingDraft(item, draft);
  if (validation.error !== null || validation.value === undefined) {
    return true;
  }
  if (item.overrideValue === null) {
    return true;
  }
  return !settingValuesEqual(validation.value, item.overrideValue);
}

function settingValuesEqual(left: SettingValue, right: SettingValue) {
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left)
      && Array.isArray(right)
      && left.length === right.length
      && left.every((value, index) => value === right[index]);
  }
  return left === right;
}

function invalid(error: string): SettingDraftValidation {
  return { value: undefined, error };
}

function isModelAccessDraft(value: SettingDraft): value is ModelAccessDraft {
  return typeof value === "object"
    && value !== null
    && (value.mode === "all" || value.mode === "only")
    && Array.isArray(value.models);
}
