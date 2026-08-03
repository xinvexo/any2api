import { act, render } from "@testing-library/react";
import type { ChartConfiguration } from "chart.js";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const chartProbe = vi.hoisted(() => ({
  instances: [] as Array<{
    data: unknown;
    destroy: ReturnType<typeof vi.fn>;
    options: unknown;
    update: ReturnType<typeof vi.fn>;
  }>,
}));

vi.mock("chart.js", () => {
  class ChartMock {
    static register = vi.fn();

    data: unknown;
    destroy = vi.fn();
    options: unknown;
    update = vi.fn();

    constructor(
      _canvas: HTMLCanvasElement,
      configuration: { data: unknown; options?: unknown },
    ) {
      this.data = configuration.data;
      this.options = configuration.options ?? {};
      chartProbe.instances.push(this);
    }
  }

  class ChartPartMock {}

  return {
    ArcElement: ChartPartMock,
    CategoryScale: ChartPartMock,
    Chart: ChartMock,
    DoughnutController: ChartPartMock,
    Filler: ChartPartMock,
    Legend: ChartPartMock,
    LinearScale: ChartPartMock,
    LineController: ChartPartMock,
    LineElement: ChartPartMock,
    PieController: ChartPartMock,
    PointElement: ChartPartMock,
    Tooltip: ChartPartMock,
  };
});

import { OverviewChart } from "./OverviewChart";

const INITIAL_CONFIGURATION = {
  type: "line",
  data: { labels: ["first"], datasets: [{ data: [1] }] },
  options: { responsive: true },
} satisfies ChartConfiguration<"line", number[], string>;

let animationFrames: FrameRequestCallback[];

beforeEach(() => {
  animationFrames = [];
  chartProbe.instances.length = 0;
  vi.stubGlobal("CanvasRenderingContext2D", class CanvasRenderingContext2DMock {});
  vi.stubGlobal(
    "requestAnimationFrame",
    vi.fn((callback: FrameRequestCallback) => {
      animationFrames.push(callback);
      return animationFrames.length;
    }),
  );
  vi.stubGlobal("cancelAnimationFrame", vi.fn());
  vi.spyOn(window.navigator, "userAgent", "get").mockReturnValue("test-browser");
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    bottom: 320,
    height: 320,
    left: 0,
    right: 640,
    top: 0,
    width: 640,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  document.documentElement.removeAttribute("data-theme");
});

afterEach(() => {
  document.documentElement.removeAttribute("data-theme");
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("updates data without rebuilding and rebuilds only when the theme changes", async () => {
  const firstConfiguration = vi.fn(() => INITIAL_CONFIGURATION);
  const rendered = render(
    <OverviewChart<"line">
      ariaLabel="调用曲线"
      createConfiguration={firstConfiguration}
    />,
  );

  expect(chartProbe.instances).toHaveLength(0);
  act(() => animationFrames.shift()?.(0));

  expect(chartProbe.instances).toHaveLength(1);
  const originalChart = chartProbe.instances[0];
  expect(originalChart.update).not.toHaveBeenCalled();

  const refreshedConfiguration = {
    type: "line",
    data: { labels: ["second"], datasets: [{ data: [2] }] },
    options: { responsive: false },
  } satisfies ChartConfiguration<"line", number[], string>;
  rendered.rerender(
    <OverviewChart<"line">
      ariaLabel="调用曲线"
      createConfiguration={() => refreshedConfiguration}
    />,
  );

  expect(chartProbe.instances).toHaveLength(1);
  expect(originalChart.destroy).not.toHaveBeenCalled();
  expect(originalChart.data).toBe(refreshedConfiguration.data);
  expect(originalChart.options).toBe(refreshedConfiguration.options);
  expect(originalChart.update).toHaveBeenCalledOnce();
  expect(originalChart.update).toHaveBeenCalledWith("none");

  await act(async () => {
    document.documentElement.setAttribute("data-theme", "dark");
    await Promise.resolve();
  });

  expect(originalChart.destroy).toHaveBeenCalledOnce();
  expect(chartProbe.instances).toHaveLength(2);
  const themedChart = chartProbe.instances[1];
  expect(themedChart.data).toBe(refreshedConfiguration.data);

  rendered.unmount();
  expect(themedChart.destroy).toHaveBeenCalledOnce();

  await act(async () => {
    document.documentElement.setAttribute("data-theme", "light");
    await Promise.resolve();
  });
  expect(chartProbe.instances).toHaveLength(2);
});
