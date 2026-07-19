export type SettingValueType = "boolean" | "integer" | "duration_ms" | "enum";
export type SettingApplyMode = "hot_reload" | "restart_required";
export type SettingValue = boolean | number | string;

export interface SettingItem {
  key: string;
  valueType: SettingValueType;
  defaultValue: SettingValue;
  overrideValue: SettingValue | null;
  effectiveValue: SettingValue;
  minValue: number | null;
  maxValue: number | null;
  allowedValues: string[] | null;
  applyMode: SettingApplyMode;
  webGroup: string;
  description: string;
}

export interface SettingsConfiguration {
  configRevision: number;
  items: SettingItem[];
}

export interface SettingWriteInput {
  expectedRevision: number;
  value: SettingValue;
}

export function parseSettingsConfiguration(value: unknown): SettingsConfiguration {
  if (!isRecord(value) || !isSafePositiveInteger(value.config_revision) || !Array.isArray(value.items)) {
    throw invalidResponse();
  }
  const items = value.items.map(parseSettingItem);
  if (new Set(items.map((item) => item.key)).size !== items.length) {
    throw invalidResponse();
  }
  return {
    configRevision: value.config_revision,
    items,
  };
}

function parseSettingItem(value: unknown): SettingItem {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const valueType = readValueType(value.value_type);
  const allowedValues = readAllowedValues(value.allowed_values, valueType);
  const minValue = readBound(value.min_value, valueType);
  const maxValue = readBound(value.max_value, valueType);
  if (minValue !== null && maxValue !== null && minValue > maxValue) {
    throw invalidResponse();
  }
  const defaultValue = readSettingValue(value.default_value, valueType, allowedValues);
  const overrideValue = value.override_value === null
    ? null
    : readSettingValue(value.override_value, valueType, allowedValues);
  const effectiveValue = readSettingValue(value.effective_value, valueType, allowedValues);
  validateRange(defaultValue, minValue, maxValue);
  validateRange(overrideValue, minValue, maxValue);
  validateRange(effectiveValue, minValue, maxValue);
  if (!settingValuesEqual(effectiveValue, overrideValue ?? defaultValue)) {
    throw invalidResponse();
  }
  return {
    key: readString(value.key),
    valueType,
    defaultValue,
    overrideValue,
    effectiveValue,
    minValue,
    maxValue,
    allowedValues,
    applyMode: readApplyMode(value.apply_mode),
    webGroup: readString(value.web_group),
    description: readString(value.description),
  };
}

function readAllowedValues(value: unknown, valueType: SettingValueType) {
  if (valueType !== "enum") {
    if (value !== null) {
      throw invalidResponse();
    }
    return null;
  }
  const values = readStringArray(value);
  if (values.length === 0 || new Set(values).size !== values.length) {
    throw invalidResponse();
  }
  return values;
}

function readBound(value: unknown, valueType: SettingValueType) {
  const numeric = valueType === "integer" || valueType === "duration_ms";
  if (!numeric) {
    if (value !== null) {
      throw invalidResponse();
    }
    return null;
  }
  return value === null ? null : readSafeNonNegativeInteger(value);
}

function readSettingValue(
  value: unknown,
  valueType: SettingValueType,
  allowedValues: string[] | null,
): SettingValue {
  if (valueType === "boolean") {
    return readBoolean(value);
  }
  if (valueType === "enum") {
    const text = readString(value);
    if (!allowedValues?.includes(text)) {
      throw invalidResponse();
    }
    return text;
  }
  return readSafeNonNegativeInteger(value);
}

function validateRange(value: SettingValue | null, minValue: number | null, maxValue: number | null) {
  if (typeof value !== "number") {
    return;
  }
  if ((minValue !== null && value < minValue) || (maxValue !== null && value > maxValue)) {
    throw invalidResponse();
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) {
    throw invalidResponse();
  }
  return value;
}

function readStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    throw invalidResponse();
  }
  return value.map(readString);
}

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw invalidResponse();
  }
  return value;
}

function readSafeNonNegativeInteger(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw invalidResponse();
  }
  return value;
}

function isSafePositiveInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

function readValueType(value: unknown): SettingValueType {
  if (value !== "boolean" && value !== "integer" && value !== "duration_ms" && value !== "enum") {
    throw invalidResponse();
  }
  return value;
}

function readApplyMode(value: unknown): SettingApplyMode {
  if (value !== "hot_reload" && value !== "restart_required") {
    throw invalidResponse();
  }
  return value;
}

function settingValuesEqual(left: SettingValue, right: SettingValue) {
  return typeof left === typeof right && left === right;
}

function invalidResponse() {
  return new Error("invalid settings response");
}
