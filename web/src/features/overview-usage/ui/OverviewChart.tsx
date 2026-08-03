import { useEffect, useEffectEvent, useRef } from "react";
import {
  ArcElement,
  CategoryScale,
  Chart as ChartJs,
  DoughnutController,
  Filler,
  Legend,
  LinearScale,
  LineController,
  LineElement,
  PieController,
  PointElement,
  Tooltip,
  type ChartConfiguration,
  type ChartType,
} from "chart.js";

import { cn } from "@/shared/lib/cn";

ChartJs.register(
  ArcElement,
  CategoryScale,
  DoughnutController,
  Filler,
  Legend,
  LinearScale,
  LineController,
  LineElement,
  PieController,
  PointElement,
  Tooltip,
);

const CHART_COLOR_TOKENS = [
  "--chart-1",
  "--chart-2",
  "--chart-3",
  "--chart-4",
  "--chart-5",
  "--chart-6",
  "--chart-7",
  "--chart-8",
] as const;

export interface OverviewChartPalette {
  borderSubtle: string;
  chartColors: string[];
  surface: string;
  textPrimary: string;
  textSecondary: string;
  textTertiary: string;
}

export function OverviewChart<TType extends ChartType>({
  ariaLabel,
  className,
  createConfiguration,
}: {
  ariaLabel: string;
  className?: string;
  createConfiguration: (
    palette: OverviewChartPalette,
  ) => ChartConfiguration<TType, number[], string>;
}) {
  const hostRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const chartRef = useRef<ChartJs<TType, number[], string> | null>(null);
  const createLatestConfiguration = useEffectEvent(() => createConfiguration(readPalette()));

  useEffect(() => {
    const host = hostRef.current;
    const canvas = canvasRef.current;
    if (!host || !canvas) return;

    const renderChart = () => {
      chartRef.current?.destroy();
      chartRef.current = null;
      if (
        host.getBoundingClientRect().width <= 0 ||
        typeof CanvasRenderingContext2D === "undefined" ||
        navigator.userAgent.toLowerCase().includes("jsdom")
      ) {
        return;
      }
      chartRef.current = new ChartJs(canvas, createLatestConfiguration());
    };

    const frame = requestAnimationFrame(renderChart);
    const themeObserver = new MutationObserver(renderChart);
    themeObserver.observe(document.documentElement, {
      attributeFilter: ["data-theme"],
      attributes: true,
    });
    return () => {
      cancelAnimationFrame(frame);
      themeObserver.disconnect();
      chartRef.current?.destroy();
      chartRef.current = null;
    };
  }, []);

  useEffect(() => {
    const chart = chartRef.current;
    if (!chart) return;
    const configuration = createConfiguration(readPalette());
    chart.data = configuration.data;
    if (configuration.options !== undefined) {
      chart.options = configuration.options;
    }
    chart.update("none");
  }, [createConfiguration]);

  return (
    <div
      ref={hostRef}
      className={cn("relative min-w-0", className)}
      role="img"
      aria-label={ariaLabel}
    >
      <canvas ref={canvasRef} aria-hidden="true" />
    </div>
  );
}

function readPalette(): OverviewChartPalette {
  const styles = getComputedStyle(document.documentElement);
  return {
    borderSubtle: readToken(styles, "--border-subtle"),
    chartColors: CHART_COLOR_TOKENS.map((token) => readToken(styles, token)),
    surface: readToken(styles, "--surface"),
    textPrimary: readToken(styles, "--text-primary"),
    textSecondary: readToken(styles, "--text-secondary"),
    textTertiary: readToken(styles, "--text-tertiary"),
  };
}

function readToken(styles: CSSStyleDeclaration, token: string) {
  return styles.getPropertyValue(token).trim();
}
