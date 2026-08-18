import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, expect, test, vi } from "vitest";

import { parseOverviewUsage, type OverviewUsage } from "../api/overview-usage-contracts";
import { overviewUsageWire } from "../api/overview-usage-test-support";

const probe = vi.hoisted(() => ({ data: undefined as OverviewUsage | undefined }));

vi.mock("../model/use-overview-usage", () => ({
  useOverviewUsage: () => ({
    data: probe.data,
    error: null,
    isError: false,
    isFetching: false,
    isPending: false,
    refetch: vi.fn(),
  }),
}));

vi.mock("./OverviewCharts", () => ({
  OverviewCharts: () => <div data-testid="overview-charts" />,
}));

import { OverviewChartsLoading, OverviewUsageSection } from "./OverviewUsageSection";

beforeEach(() => {
  probe.data = parseOverviewUsage(overviewUsageWire());
});

test("shows the selected range prompt cache hit rate", async () => {
  render(
    <MemoryRouter>
      <OverviewUsageSection />
    </MemoryRouter>,
  );

  expect(screen.getByText("缓存命中率")).toBeInTheDocument();
  expect(screen.getByText("40%")).toBeInTheDocument();
  expect(screen.getByText("缓存读取 4 / 输入 10")).toBeInTheDocument();
  expect(await screen.findByTestId("overview-charts")).toBeInTheDocument();
});

test("shows an unknown cache hit rate without input tokens", () => {
  if (!probe.data) throw new Error("overview fixture missing");
  probe.data.selected.inputTokens = 0n;
  probe.data.selected.cacheReadTokens = 0n;

  render(
    <MemoryRouter>
      <OverviewUsageSection />
    </MemoryRouter>,
  );

  expect(screen.getByText("暂无输入 Token")).toBeInTheDocument();
});

test("uses quiet borderless surfaces while charts are loading", () => {
  render(<OverviewChartsLoading />);

  const surfaces = screen.getByRole("status", { name: "正在加载调用图表" })
    .querySelectorAll("section");
  expect(surfaces).toHaveLength(2);
  for (const surface of surfaces) {
    expect(surface).toHaveClass("rounded-[14px]", "bg-surface-muted/45");
    expect(surface).not.toHaveClass("border", "border-subtle");
  }
});
