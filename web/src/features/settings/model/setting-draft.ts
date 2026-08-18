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

export function sanitizeIntegerDraft(value: string) {
  return value.replace(/\D+/gu, "");
}

export function createSettingDraft(item: SettingItem): SettingDraft {
  return createSettingDraftFromValue(item, item.effectiveValue);
}

export function createSettingDraftFromValue(
  item: SettingItem,
  value: SettingValue,
): SettingDraft {
  if (item.valueType === "model_access") {
    if (value === "all") {
      return { mode: "all", models: [] };
    }
    if (Array.isArray(value)) {
      return { mode: "only", models: [...value] };
    }
    throw new Error("invalid model access setting");
  }
  if (item.valueType === "string_list") {
    if (Array.isArray(value)) {
      return value.join("\n");
    }
    throw new Error("invalid string list setting");
  }
  if (typeof value === "number") {
    return String(value);
  }
  if (typeof value === "boolean" || typeof value === "string") {
    return value;
  }
  throw new Error("invalid setting value");
}

export function validateSettingDraft(
  item: SettingItem,
  draft: SettingDraft,
): SettingDraftValidation {
  if (item.valueType === "model_access") {
    if (!isModelAccessDraft(draft)) {
      return invalid("模型选择格式不正确");
    }
    if (draft.mode === "all") {
      return { value: "all", error: null };
    }
    const values = [...new Set(draft.models)].sort();
    if (values.some((value) => !item.options?.includes(value))) {
      return invalid("模型列表已发生变化，请刷新后重试");
    }
    return { value: values, error: null };
  }
  if (item.valueType === "string_list") {
    if (typeof draft !== "string") {
      return invalid("地址列表格式不正确");
    }
    const values = [...new Set(
      draft
        .split(/[\n,]/u)
        .map((value) => value.trim())
        .filter(Boolean),
    )].sort();
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
  return !settingValuesEqual(validation.value, item.effectiveValue);
}

function settingValuesEqual(left: SettingValue, right: SettingValue): boolean {
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left)
      && Array.isArray(right)
      && left.length === right.length
      && left.every((value, index) => settingValuesEqual(value, right[index]));
  }
  if (
    (typeof left === "object" && left !== null)
    || (typeof right === "object" && right !== null)
  ) {
    if (typeof left !== "object" || left === null || typeof right !== "object" || right === null) {
      return false;
    }
    const leftRecord = left as unknown as Record<string, SettingValue>;
    const rightRecord = right as unknown as Record<string, SettingValue>;
    const leftKeys = Object.keys(leftRecord).sort();
    const rightKeys = Object.keys(rightRecord).sort();
    return leftKeys.length === rightKeys.length
      && leftKeys.every((key, index) =>
        key === rightKeys[index]
        && settingValuesEqual(leftRecord[key], rightRecord[key])
      );
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
