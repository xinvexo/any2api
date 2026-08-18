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

import { OverviewUsageSection } from "./OverviewUsageSection";

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
