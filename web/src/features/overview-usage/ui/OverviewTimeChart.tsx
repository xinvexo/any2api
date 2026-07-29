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
            backgroundColor: chartColorWithAlpha(palette.chartColors[0], 0.12),
            borderColor: palette.chartColors[0],
            borderWidth: 2.25,
            cubicInterpolationMode: "monotone",
            data: buckets.map((bucket) => bucket.requestCount),
            fill: "origin",
            label: "总调用",
            pointBackgroundColor: palette.chartColors[0],
            pointBorderColor: palette.chartColors[0],
            pointBorderWidth: 0,
            pointHoverBackgroundColor: palette.chartColors[0],
            pointHoverBorderColor: palette.surface,
            pointHoverBorderWidth: 2,
            pointHoverRadius: 5,
            pointRadius: 0,
            pointStyle: "circle",
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
            pointBackgroundColor: palette.chartColors[3],
            pointBorderColor: palette.chartColors[3],
            pointBorderWidth: 0,
            pointHoverBackgroundColor: palette.chartColors[3],
            pointHoverBorderColor: palette.surface,
            pointHoverBorderWidth: 2,
            pointHoverRadius: 5,
            pointRadius: 0,
            pointStyle: "circle",
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
            align: "end",
            labels: {
              boxHeight: 8,
              boxWidth: 8,
              color: palette.textSecondary,
              font: { family: "inherit", size: 11, weight: 500 },
              generateLabels: (chart) => {
                const datasets = chart.data.datasets ?? [];
                return datasets.map((dataset, index) => {
                  const color = String(dataset.borderColor ?? palette.chartColors[0]);
                  return {
                    text: String(dataset.label ?? ""),
                    fillStyle: color,
                    strokeStyle: color,
                    lineWidth: 0,
                    hidden: !chart.isDatasetVisible(index),
                    datasetIndex: index,
                    pointStyle: "circle" as const,
                  };
                });
              },
              padding: 14,
              pointStyle: "circle",
              usePointStyle: true,
            },
            position: "top",
          },
          tooltip: {
            backgroundColor: palette.surface,
            bodyColor: palette.textSecondary,
            bodySpacing: 6,
            borderColor: palette.borderSubtle,
            borderWidth: 1,
            boxHeight: 8,
            boxPadding: 6,
            boxWidth: 8,
            callbacks: {
              label: (context) =>
                ` ${context.dataset.label}  ${formatOverviewInteger(context.parsed.y ?? 0)} 次`,
              labelColor: (context) => {
                const color = String(context.dataset.borderColor ?? palette.chartColors[0]);
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
            titleMarginBottom: 8,
            usePointStyle: true,
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
              font: { family: "inherit", size: 10 },
              maxRotation: 0,
              padding: 8,
            },
          },
          y: {
            beginAtZero: true,
            border: { display: false },
            grid: {
              color: palette.borderSubtle,
              drawTicks: false,
            },
            max: yMaximum,
            ticks: {
              color: palette.textTertiary,
              font: { family: "inherit", size: 10 },
              padding: 8,
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
    <div className="min-w-0" data-testid="overview-time-chart">
      <div role="group" aria-label={`按时间展示的 ${buckets.length} 个调用桶`}>
        <OverviewChart
          ariaLabel={`近 ${buckets.length} 个时间桶的总调用与失败调用曲线`}
          className="h-72 w-full"
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
