type SettingValueType =
  | "boolean"
  | "integer"
  | "duration_secs"
  | "enum"
  | "model_access"
  | "string_list"
  | "codex_rate_card";
type SettingApplyMode = "hot_reload" | "restart_required";
export type SettingValue = boolean | number | string | string[] | CodexRateCardValue;

export interface CodexRateCardValue {
  id: string;
  credits_per_usd: number;
  models: Record<string, {
    standard: CodexRateTierValue;
    fast?: CodexRateTierValue | null;
  }>;
}

export interface CodexRateTierValue {
  input_nanos_per_million: number;
  cached_input_nanos_per_million: number;
  output_nanos_per_million: number;
}

const MAX_CODEX_RATE_CARD_ID_CHARS = 128;
const MAX_CODEX_RATE_CARD_MODELS = 256;
const MAX_CODEX_MODEL_NAME_CHARS = 255;
const MAX_CODEX_CREDITS_PER_USD = 1_000_000;
const MAX_CODEX_RATE_NANOS_PER_MILLION = 9_000_000_000_000_000;

export interface SettingItem {
  key: string;
  valueType: SettingValueType;
  defaultValue: SettingValue;
  overrideValue: SettingValue | null;
  effectiveValue: SettingValue;
  minValue: number | null;
  maxValue: number | null;
  allowedValues: string[] | null;
  options: string[] | null;
  applyMode: SettingApplyMode;
  webGroup: string;
  description: string;
}

export interface SettingsConfiguration {
  configRevision: number;
  items: SettingItem[];
}

export interface SettingBatchWriteInput {
  expectedRevision: number;
  updates: Array<{ key: string; value: SettingValue }>;
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
  const options = readOptions(value.options, valueType);
  const minValue = readBound(value.min_value, valueType);
  const maxValue = readBound(value.max_value, valueType);
  if (minValue !== null && maxValue !== null && minValue > maxValue) {
    throw invalidResponse();
  }
  const defaultValue = readSettingValue(value.default_value, valueType, allowedValues, options);
  const overrideValue = value.override_value === null
    ? null
    : readSettingValue(value.override_value, valueType, allowedValues, options);
  const effectiveValue = readSettingValue(value.effective_value, valueType, allowedValues, options);
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
    options,
    applyMode: readApplyMode(value.apply_mode),
    webGroup: readString(value.web_group),
    description: readDescription(value.description),
  };
}

function readOptions(value: unknown, valueType: SettingValueType) {
  if (valueType !== "model_access" && valueType !== "string_list") {
    if (value !== null) {
      throw invalidResponse();
    }
    return null;
  }
  if (value === null) {
    return null;
  }
  const values = readStringArray(value);
  if (new Set(values).size !== values.length) {
    throw invalidResponse();
  }
  return values;
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
  const numeric = valueType === "integer" || valueType === "duration_secs";
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
  options: string[] | null,
): SettingValue {
  if (valueType === "codex_rate_card") {
    return parseCodexRateCardValue(value);
  }
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
  if (valueType === "string_list") {
    const values = readStringArray(value);
    if (
      new Set(values).size !== values.length
      || (options !== null && values.some((item) => !options.includes(item)))
    ) {
      throw invalidResponse();
    }
    return values;
  }
  if (valueType === "model_access") {
    if (value === "all") {
      return value;
    }
    const values = readStringArray(value);
    if (
      new Set(values).size !== values.length
      || (options !== null && values.some((item) => !options.includes(item)))
    ) {
      throw invalidResponse();
    }
    return values;
  }
  return readSafeNonNegativeInteger(value);
}

export function parseCodexRateCardValue(value: unknown): CodexRateCardValue {
  if (!isRecord(value)
    || Array.isArray(value)
    || !isRecord(value.models)
    || Array.isArray(value.models)
    || !hasOnlyKeys(value, ["id", "credits_per_usd", "models"])
  ) {
    throw invalidResponse();
  }
  const id = readBoundedIdentifier(value.id, MAX_CODEX_RATE_CARD_ID_CHARS);
  const creditsPerUsd = readSafePositiveInteger(value.credits_per_usd);
  if (creditsPerUsd > MAX_CODEX_CREDITS_PER_USD) throw invalidResponse();
  const entries = Object.entries(value.models);
  if (entries.length === 0 || entries.length > MAX_CODEX_RATE_CARD_MODELS) {
    throw invalidResponse();
  }
  const models: CodexRateCardValue["models"] = {};
  for (const [model, raw] of entries) {
    if (!isRecord(raw)
      || !hasOnlyKeys(raw, ["standard", "fast"], ["standard"])
      || !validBoundedIdentifier(model, MAX_CODEX_MODEL_NAME_CHARS)
    ) {
      throw invalidResponse();
    }
    models[model] = {
      standard: readCodexRateTier(raw.standard),
      fast: raw.fast === null || raw.fast === undefined ? raw.fast : readCodexRateTier(raw.fast),
    };
  }
  return { id, credits_per_usd: creditsPerUsd, models };
}

function readCodexRateTier(value: unknown): CodexRateTierValue {
  if (!isRecord(value) || !hasOnlyKeys(value, [
    "input_nanos_per_million",
    "cached_input_nanos_per_million",
    "output_nanos_per_million",
  ])) {
    throw invalidResponse();
  }
  const tier = {
    input_nanos_per_million: readSafePositiveInteger(value.input_nanos_per_million),
    cached_input_nanos_per_million: readSafeNonNegativeInteger(value.cached_input_nanos_per_million),
    output_nanos_per_million: readSafePositiveInteger(value.output_nanos_per_million),
  };
  if (tier.cached_input_nanos_per_million > tier.input_nanos_per_million
    || Object.values(tier).some((rate) => rate > MAX_CODEX_RATE_NANOS_PER_MILLION)
  ) {
    throw invalidResponse();
  }
  return tier;
}

function hasOnlyKeys(
  value: Record<string, unknown>,
  allowed: string[],
  required: string[] = allowed,
) {
  const keys = Object.keys(value);
  return required.every((key) => keys.includes(key))
    && keys.every((key) => allowed.includes(key));
}

function readBoundedIdentifier(value: unknown, maximumCharacters: number) {
  if (typeof value !== "string" || !validBoundedIdentifier(value, maximumCharacters)) {
    throw invalidResponse();
  }
  return value;
}

function validBoundedIdentifier(value: string, maximumCharacters: number) {
  return value.length > 0
    && value.trim() === value
    && [...value].length <= maximumCharacters
    && !/\p{Cc}/u.test(value);
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

/** Setting help text may be empty when the UI only needs a label. */
function readDescription(value: unknown): string {
  if (typeof value !== "string") {
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
  if (
    value !== "boolean"
    && value !== "integer"
    && value !== "duration_secs"
    && value !== "enum"
    && value !== "model_access"
    && value !== "string_list"
    && value !== "codex_rate_card"
  ) {
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

function settingValuesEqual(left: SettingValue, right: SettingValue): boolean {
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left)
      && Array.isArray(right)
      && left.length === right.length
      && left.every((value, index) => settingValuesEqual(value, right[index]));
  }
  if (isRecord(left) || isRecord(right)) {
    if (!isRecord(left) || !isRecord(right)) return false;
    const leftKeys = Object.keys(left).sort();
    const rightKeys = Object.keys(right).sort();
    return leftKeys.length === rightKeys.length
      && leftKeys.every((key, index) =>
        key === rightKeys[index]
        && settingValuesEqual(left[key] as SettingValue, right[key] as SettingValue)
      );
  }
  return left === right;
}

function readSafePositiveInteger(value: unknown) {
  const number = readSafeNonNegativeInteger(value);
  if (number === 0) throw invalidResponse();
  return number;
}

function invalidResponse() {
  return new Error("invalid settings response");
}
