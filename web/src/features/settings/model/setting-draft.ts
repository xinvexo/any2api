import type { SettingItem, SettingValue } from "../api/settings-contracts";

export type SettingDraft = boolean | string;

export interface SettingDraftValidation {
  value: SettingValue | null;
  error: string | null;
}

export function createSettingDraft(item: SettingItem): SettingDraft {
  return typeof item.effectiveValue === "number"
    ? String(item.effectiveValue)
    : item.effectiveValue;
}

export function validateSettingDraft(
  item: SettingItem,
  draft: SettingDraft,
): SettingDraftValidation {
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
  if (validation.error !== null) {
    return true;
  }
  if (item.overrideValue === null) {
    return true;
  }
  return validation.value !== item.overrideValue;
}

function invalid(error: string): SettingDraftValidation {
  return { value: null, error };
}
