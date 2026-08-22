import type { SettingItem } from "../api/settings-contracts";

export type SettingUnitKind = "duration" | "bytes";

export interface SettingUnitOption {
  value: string;
  label: string;
  factor: number;
}

const DURATION_UNITS: readonly SettingUnitOption[] = [
  { value: "seconds", label: "秒", factor: 1 },
  { value: "minutes", label: "分钟", factor: 60 },
  { value: "hours", label: "小时", factor: 60 * 60 },
  { value: "days", label: "天", factor: 24 * 60 * 60 },
  { value: "months", label: "月", factor: 30 * 24 * 60 * 60 },
];

const BYTE_UNITS: readonly SettingUnitOption[] = [
  { value: "kb", label: "KB", factor: 1024 },
  { value: "mb", label: "MB", factor: 1024 ** 2 },
  { value: "gb", label: "GB", factor: 1024 ** 3 },
];

const BYTE_SETTING_KEYS = new Set([
  "logs.file.max_total_size",
  "logs.telemetry_queue_max_bytes",
  "stream.precommit.max_bytes",
]);

export function settingUnitKind(item: SettingItem): SettingUnitKind | null {
  if (item.valueType === "duration_secs") {
    return "duration";
  }
  return BYTE_SETTING_KEYS.has(item.key) ? "bytes" : null;
}

export function settingUnitOptions(kind: SettingUnitKind): readonly SettingUnitOption[] {
  return kind === "duration" ? DURATION_UNITS : BYTE_UNITS;
}

export function preferredSettingUnit(item: SettingItem, rawValue: number): SettingUnitOption {
  const kind = settingUnitKind(item);
  if (kind === null) {
    throw new Error("setting does not use a scaled unit");
  }
  const units = settingUnitOptions(kind);
  return [...units].reverse().find((unit) => rawValue >= unit.factor && rawValue % unit.factor === 0)
    ?? units[0];
}

export function formatSettingUnitValue(rawValue: string, unit: SettingUnitOption): string {
  if (!/^\d+$/u.test(rawValue)) {
    return rawValue;
  }
  const amount = Number(rawValue) / unit.factor;
  return formatDecimal(amount);
}

export function parseSettingUnitValue(displayValue: string, unit: SettingUnitOption): string {
  const text = displayValue.trim();
  if (text.length === 0 || !/^\d+(?:\.\d+)?$/u.test(text)) {
    return displayValue;
  }
  const rawValue = Number(text) * unit.factor;
  return Number.isSafeInteger(rawValue) ? String(rawValue) : displayValue;
}

export function sanitizeSettingUnitInput(value: string) {
  const normalized = value.replace(/[^\d.]+/gu, "");
  const [whole, ...fractionParts] = normalized.split(".");
  return fractionParts.length > 0
    ? `${whole}.${fractionParts.join("")}`
    : whole;
}

function formatDecimal(value: number) {
  return Number.isInteger(value) ? String(value) : value.toFixed(3).replace(/0+$/u, "").replace(/\.$/u, "");
}
