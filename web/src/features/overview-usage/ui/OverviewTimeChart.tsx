import { useCallback } from "react";
import type { ChartConfiguration } from "chart.js";

import type {
  OverviewUsageRange,
  OverviewUsageTimeBucket,
} from "../api/overview-usage-contracts";
import {
  formatOverviewBucketRange,
  formatOverviewBucketTime,
  formatOverviewInteger,
} from "../model/overview-usage-presentation";
import { chartColorWithAlpha } from "../model/overview-chart-color";
import { OverviewChart, type OverviewChartPalette } from "./OverviewChart";

export function OverviewTimeChart({
  buckets,
  range,
}: {
  buckets: OverviewUsageTimeBucket[];
  range: OverviewUsageRange;
}) {
  const yMaximum = Math.max(1, ...buckets.map((bucket) => bucket.requestCount)) * 1.1;
  const createConfiguration = useCallback(
    (palette: OverviewChartPalette): ChartConfiguration<"line", number[], string> => ({
      data: {
        labels: compactAxisLabels(buckets, range),
        datasets: [
          {
            backgroundColor: chartColorWithAlpha(palette.chartColors[0], 0.1),
            borderColor: palette.chartColors[0],
            borderWidth: 2.25,
            cubicInterpolationMode: "monotone",
            data: buckets.map((bucket) => bucket.requestCount),
            fill: "origin",
            label: "总调用",
            pointBackgroundColor: palette.surface,
            pointBorderColor: palette.chartColors[0],
            pointBorderWidth: 2,
            pointHoverRadius: 5,
            pointRadius: 2.5,
            tension: 0.35,
          },
          {
            backgroundColor: chartColorWithAlpha(palette.chartColors[3], 0.08),
            borderColor: palette.chartColors[3],
            borderWidth: 2,
            cubicInterpolationMode: "monotone",
            data: buckets.map((bucket) => bucket.failedRequestCount),
            fill: false,
            label: "失败",
            pointBackgroundColor: palette.surface,
            pointBorderColor: palette.chartColors[3],
            pointBorderWidth: 2,
            pointHoverRadius: 5,
            pointRadius: 2.5,
            tension: 0.35,
          },
        ],
      },
      options: {
        animation: { duration: 550, easing: "easeOutQuart" },
        interaction: { intersect: false, mode: "index" },
        maintainAspectRatio: false,
        normalized: true,
        plugins: {
          legend: {
            align: "start",
            labels: {
              boxHeight: 2,
              boxWidth: 18,
              color: palette.textSecondary,
              font: { family: "inherit", size: 10, weight: 500 },
              padding: 16,
              pointStyle: "line",
              usePointStyle: true,
            },
            position: "bottom",
          },
          tooltip: {
            backgroundColor: palette.surface,
            borderColor: palette.borderSubtle,
            borderWidth: 1,
            bodyColor: palette.textSecondary,
            callbacks: {
              label: (context) =>
                `${context.dataset.label}: ${formatOverviewInteger(context.parsed.y ?? 0)} 次`,
            },
            cornerRadius: 9,
            displayColors: true,
            padding: 10,
            titleColor: palette.textPrimary,
          },
        },
        responsive: true,
        scales: {
          x: {
            border: { display: false },
            grid: { display: false },
            ticks: {
              autoSkip: false,
              color: palette.textTertiary,
              font: { family: "inherit", size: 9 },
              maxRotation: 0,
              padding: 6,
            },
          },
          y: {
            beginAtZero: true,
            border: { display: false },
            grid: { color: palette.borderSubtle },
            max: yMaximum,
            ticks: {
              color: palette.textTertiary,
              font: { family: "inherit", size: 9 },
              precision: 0,
            },
          },
        },
      },
      type: "line",
    }),
    [buckets, range, yMaximum],
  );

  return (
    <div className="mt-5 min-w-0" data-testid="overview-time-chart">
      <p className="text-[11px] text-secondary">平滑曲线分别展示总调用与失败调用，悬浮数据点可查看明细。</p>
      <div className="mt-2" role="group" aria-label={`按时间展示的 ${buckets.length} 个调用桶`}>
        <OverviewChart
          ariaLabel={`近 ${buckets.length} 个时间桶的总调用与失败调用曲线`}
          className="h-60 w-full"
          createConfiguration={createConfiguration}
        />
      </div>
      <ol className="sr-only">
        {buckets.map((bucket) => (
          <li key={bucket.startedAtMs}>
            {formatOverviewBucketRange(bucket.startedAtMs, bucket.endedAtMs)}：调用{" "}
            {formatOverviewInteger(bucket.requestCount)}，成功{" "}
            {formatOverviewInteger(bucket.successfulRequestCount)}，失败{" "}
            {formatOverviewInteger(bucket.failedRequestCount)}
          </li>
        ))}
      </ol>
    </div>
  );
}

function compactAxisLabels(buckets: OverviewUsageTimeBucket[], range: OverviewUsageRange) {
  const stride = Math.max(1, Math.ceil(buckets.length / 6));
  return buckets.map((bucket, index) =>
    index === 0 || index === buckets.length - 1 || index % stride === 0
      ? formatOverviewBucketTime(bucket.startedAtMs, range)
      : "",
  );
}
