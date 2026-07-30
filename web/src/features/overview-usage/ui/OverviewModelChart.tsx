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
    (palette: OverviewChartPalette): ChartConfiguration<"doughnut", number[], string> => ({
      data: {
        labels: segments.map((segment) => segment.label),
        datasets: [
          {
            backgroundColor: palette.chartColors.slice(0, segments.length),
            borderColor: palette.surface,
            borderRadius: 4,
            borderWidth: 2,
            data: segments.map((segment) => segment.requestCount),
            hoverOffset: 4,
            spacing: 1,
          },
        ],
      },
      options: {
        animation: { duration: 550, easing: "easeOutQuart" },
        cutout: "62%",
        layout: { padding: 2 },
        maintainAspectRatio: false,
        plugins: {
          legend: { display: false },
          tooltip: {
            backgroundColor: palette.surface,
            bodyColor: palette.textSecondary,
            borderColor: palette.borderSubtle,
            borderWidth: 1,
            boxHeight: 8,
            boxPadding: 6,
            boxWidth: 8,
            callbacks: {
              label: (context) => {
                const value = context.parsed;
                const share = total === 0 ? 0 : (value / total) * 100;
                return ` ${formatOverviewInteger(value)} 次 · ${formatShare(share)}`;
              },
              labelColor: (context) => {
                const colors = context.dataset.backgroundColor;
                const color = Array.isArray(colors)
                  ? String(colors[context.dataIndex] ?? palette.chartColors[0])
                  : String(colors ?? palette.chartColors[0]);
                return {
                  backgroundColor: color,
                  borderColor: color,
                  borderWidth: 0,
                  borderRadius: 99,
                };
              },
            },
            cornerRadius: 10,
            displayColors: true,
            padding: { top: 10, right: 12, bottom: 10, left: 10 },
            titleColor: palette.textPrimary,
            titleFont: { family: "inherit", size: 11, weight: 600 },
            usePointStyle: true,
          },
        },
        responsive: true,
      },
      type: "doughnut",
    }),
    [segments, total],
  );

  if (models.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-center text-sm text-tertiary">
        当前时间段还没有模型调用。
      </div>
    );
  }

  const chartLabel = `模型调用占比：${segments
    .map((segment) => `${segment.label} ${formatOverviewInteger(segment.requestCount)} 次`)
    .join("，")}`;

  return (
    <div className="h-full min-w-0" data-testid="overview-model-chart">
      <div className="relative mx-auto w-full max-w-[168px]">
        <OverviewChart
          ariaLabel={chartLabel}
          className="h-[136px] w-full"
          createConfiguration={createConfiguration}
        />
        <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
          <p className="text-lg font-semibold leading-5 tabular-nums tracking-tight text-primary">
            {formatOverviewInteger(total)}
          </p>
          <p className="mt-0.5 text-[11px] leading-4 text-tertiary">次调用</p>
        </div>
      </div>

      <ul className="mt-3 space-y-1" aria-label="模型调用占比图例">
        {segments.map((segment) => (
          <li
            key={segment.key}
            className="min-w-0"
            title={`${segment.label}：${formatOverviewInteger(segment.requestCount)} 次，${formatShare(segment.percentage)}`}
          >
            <div className="flex items-center justify-between gap-2 text-[11px] leading-[14px]">
              <div className="flex min-w-0 items-center gap-2">
                <span
                  className="size-2 shrink-0 rounded-full"
                  style={{ backgroundColor: segment.color }}
                  aria-hidden="true"
                />
                <span className="truncate font-medium">{segment.label}</span>
              </div>
              <span className="shrink-0 tabular-nums text-tertiary">
                {formatShare(segment.percentage)}
              </span>
            </div>
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
