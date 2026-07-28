import { useCallback, useMemo } from "react";
import type { ChartConfiguration } from "chart.js";

import type { OverviewUsageModel } from "../api/overview-usage-contracts";
import {
  formatOverviewInteger,
  overviewModelLabel,
} from "../model/overview-usage-presentation";
import { OverviewChart, type OverviewChartPalette } from "./OverviewChart";

const MAX_PIE_SLICES = 8;
const MODEL_COLOR_TOKENS = Array.from({ length: MAX_PIE_SLICES }, (_, index) =>
  `var(--chart-${index + 1})`,
);

interface ModelSlice {
  key: string;
  label: string;
  requestCount: number;
}

interface ModelChartDatum extends ModelSlice {
  color: string;
  percentage: number;
}

export function OverviewModelChart({ models }: { models: OverviewUsageModel[] }) {
  const segments = useMemo<ModelChartDatum[]>(() => {
    const slices = compactModelSlices(models);
    const requestCount = slices.reduce((sum, slice) => sum + slice.requestCount, 0);
    return slices.map((slice, index) => ({
      ...slice,
      color: MODEL_COLOR_TOKENS[index],
      percentage: requestCount === 0 ? 0 : (slice.requestCount / requestCount) * 100,
    }));
  }, [models]);
  const total = segments.reduce((sum, segment) => sum + segment.requestCount, 0);
  const createConfiguration = useCallback(
    (palette: OverviewChartPalette): ChartConfiguration<"pie", number[], string> => ({
      data: {
        labels: segments.map((segment) => segment.label),
        datasets: [
          {
            backgroundColor: palette.chartColors.slice(0, segments.length),
            borderColor: palette.surface,
            borderRadius: 3,
            borderWidth: 2,
            data: segments.map((segment) => segment.requestCount),
            hoverOffset: 6,
            spacing: 1,
          },
        ],
      },
      options: {
        animation: { duration: 550, easing: "easeOutQuart" },
        layout: { padding: 8 },
        maintainAspectRatio: false,
        plugins: {
          legend: { display: false },
          tooltip: {
            backgroundColor: palette.surface,
            borderColor: palette.borderSubtle,
            borderWidth: 1,
            bodyColor: palette.textSecondary,
            callbacks: {
              label: (context) => {
                const value = context.parsed;
                const share = total === 0 ? 0 : (value / total) * 100;
                return `${formatOverviewInteger(value)} 次 · ${formatShare(share)}`;
              },
            },
            cornerRadius: 9,
            displayColors: true,
            padding: 10,
            titleColor: palette.textPrimary,
          },
        },
        responsive: true,
      },
      type: "pie",
    }),
    [segments, total],
  );

  if (models.length === 0) {
    return <p className="mt-6 text-xs text-tertiary">当前时间段还没有模型调用。</p>;
  }

  const chartLabel = `模型调用占比：${segments
    .map((segment) => `${segment.label} ${formatOverviewInteger(segment.requestCount)} 次`)
    .join("，")}`;

  return (
    <div className="mt-3 min-w-0" data-testid="overview-model-chart">
      <div className="mx-auto w-full max-w-[264px] text-center">
        <OverviewChart
          ariaLabel={chartLabel}
          className="h-[190px] w-full"
          createConfiguration={createConfiguration}
        />
        <p className="mt-1 text-[11px] text-secondary">
          <strong className="font-semibold tabular-nums text-primary">
            {formatOverviewInteger(total)}
          </strong>{" "}
          次调用
        </p>
      </div>

      <ul
        className="mt-4 grid min-w-0 grid-cols-2 gap-x-3 gap-y-2"
        aria-label="模型调用占比图例"
      >
        {segments.map((segment) => (
          <li
            key={segment.key}
            className="flex min-w-0 items-start gap-1.5"
            title={`${segment.label}：${formatOverviewInteger(segment.requestCount)} 次，${formatShare(segment.percentage)}`}
          >
            <span
              className="mt-0.5 size-2.5 shrink-0 rounded-[3px]"
              style={{ backgroundColor: segment.color }}
              aria-hidden="true"
            />
            <span className="min-w-0">
              <span className="block truncate text-[10px] font-medium">{segment.label}</span>
              <span className="mt-px block text-[9px] tabular-nums text-tertiary">
                {formatOverviewInteger(segment.requestCount)} 次 · {formatShare(segment.percentage)}
              </span>
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function compactModelSlices(models: OverviewUsageModel[]): ModelSlice[] {
  if (models.length <= MAX_PIE_SLICES) return models.map(toModelSlice);
  const visible = models.slice(0, MAX_PIE_SLICES - 1).map(toModelSlice);
  const remaining = models.slice(MAX_PIE_SLICES - 1);
  return [
    ...visible,
    {
      key: "remaining",
      label: remaining.some((model) => model.isOther)
        ? "其余模型"
        : `其余 ${remaining.length} 个模型`,
      requestCount: remaining.reduce((sum, model) => sum + model.requestCount, 0),
    },
  ];
}

function toModelSlice(model: OverviewUsageModel): ModelSlice {
  return {
    key: model.isOther ? "other" : model.publicModel === null ? "unknown" : `model:${model.publicModel}`,
    label: overviewModelLabel(model),
    requestCount: model.requestCount,
  };
}

function formatShare(percentage: number) {
  return `${new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 1 }).format(percentage)}%`;
}
