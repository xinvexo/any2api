import { render, screen } from "@testing-library/react";
import type { ChartConfiguration } from "chart.js";
import { expect, test, vi } from "vitest";

import type { OverviewUsageTimeBucket } from "../api/overview-usage-contracts";
import type { OverviewChartPalette } from "./OverviewChart";
import { OverviewTimeChart } from "./OverviewTimeChart";

vi.mock("./OverviewChart", () => ({
  OverviewChart: ({
    createConfiguration,
  }: {
    createConfiguration: (
      palette: OverviewChartPalette,
    ) => ChartConfiguration<"line", number[], string>;
  }) => (
    <output data-testid="time-chart-configuration">
      {JSON.stringify(
        createConfiguration({
          borderSubtle: "#ddd",
          chartColors: ["#07f", "#0a5", "#fb0", "#f25"],
          surface: "#fff",
          textPrimary: "#111",
          textSecondary: "#555",
          textTertiary: "#888",
        }),
      )}
    </output>
  ),
}));

test("uses an integer tick scale without forcing a fractional upper bound", () => {
  const buckets: OverviewUsageTimeBucket[] = [
    {
      startedAtMs: 0,
      endedAtMs: 300_000,
      requestCount: 128,
      successfulRequestCount: 100,
      failedRequestCount: 28,
    },
  ];

  render(<OverviewTimeChart buckets={buckets} range="1h" />);

  const serialized = screen.getByTestId("time-chart-configuration").textContent;
  const configuration = JSON.parse(serialized ?? "{}") as {
    options?: {
      scales?: {
        y?: { max?: number; suggestedMax?: number; ticks?: { precision?: number } };
      };
    };
  };
  const yScale = configuration.options?.scales?.y;
  expect(yScale?.max).toBeUndefined();
  expect(yScale?.suggestedMax).toBeCloseTo(140.8);
  expect(yScale?.ticks?.precision).toBe(0);
});
