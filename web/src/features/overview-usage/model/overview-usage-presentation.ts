import type {
  OverviewUsageModel,
  OverviewUsageRange,
} from "../api/overview-usage-contracts";

export const OVERVIEW_RANGE_OPTIONS: ReadonlyArray<{
  value: OverviewUsageRange;
  label: string;
}> = [
  { value: "1h", label: "1 小时" },
  { value: "24h", label: "24 小时" },
  { value: "7d", label: "7 天" },
  { value: "30d", label: "30 天" },
];

export function overviewRangeLabel(range: OverviewUsageRange) {
  return OVERVIEW_RANGE_OPTIONS.find((option) => option.value === range)?.label ?? range;
}

export function overviewModelLabel(model: OverviewUsageModel) {
  if (model.isOther) return "其他模型";
  return model.publicModel ?? "未识别模型";
}

export function formatOverviewInteger(value: number | bigint) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

export function calculateOverviewAverageRpm(
  requestCount: number,
  rangeStartedAtMs: number,
  rangeEndedAtMs: number,
) {
  const minutes = (rangeEndedAtMs - rangeStartedAtMs) / 60_000;
  return minutes > 0 ? requestCount / minutes : 0;
}

export function formatOverviewRpm(value: number) {
  const maximumFractionDigits =
    value < 0.001 ? 5 : value < 0.01 ? 4 : value < 1 ? 3 : value < 100 ? 2 : 1;
  return new Intl.NumberFormat("zh-CN", { maximumFractionDigits }).format(value);
}

export function formatOverviewDateTime(value: number) {
  return new Date(value).toLocaleString(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

export function formatOverviewBucketTime(value: number, range: OverviewUsageRange) {
  const options: Intl.DateTimeFormatOptions =
    range === "1h" || range === "24h"
      ? { hour: "2-digit", minute: "2-digit", hour12: false }
      : range === "7d"
        ? { month: "2-digit", day: "2-digit", hour: "2-digit", hour12: false }
        : { month: "2-digit", day: "2-digit" };
  return new Date(value).toLocaleString(undefined, options);
}

export function formatOverviewBucketRange(start: number, end: number) {
  return `${formatOverviewDateTime(start)} – ${formatOverviewDateTime(end)}`;
}
