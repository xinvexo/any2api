import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import type {
  OverviewUsageModel,
  OverviewUsageRange,
} from "../api/overview-usage-contracts";
import { overviewUsageWire } from "../api/overview-usage-test-support";
import { OverviewModelChart } from "./OverviewModelChart";
import { OverviewUsageSection } from "./OverviewUsageSection";

afterEach(() => vi.restoreAllMocks());

test("shows range metrics with simultaneous time and model charts", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = String(input);
    const range = (["1h", "24h", "7d", "30d"] as const).find((item) =>
      url.includes(`range=${item}`),
    ) as OverviewUsageRange | undefined;
    return new Response(JSON.stringify(overviewUsageWire(range ?? "24h")), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const rendered = render(
    <MemoryRouter initialEntries={["/?range=1h"]}>
      <QueryClientProvider client={client}>
        <OverviewUsageSection />
        <LocationProbe />
      </QueryClientProvider>
    </MemoryRouter>,
  );

  expect(await screen.findByText("15")).toBeInTheDocument();
  expect(screen.getByText("usage 覆盖 2 / 2 次")).toBeInTheDocument();
  expect(screen.getByText("0.033")).toBeInTheDocument();
  expect(screen.queryByText("输入 Token")).not.toBeInTheDocument();
  expect(screen.queryByText("输出 Token")).not.toBeInTheDocument();
  const timeChart = screen.getByTestId("overview-time-chart");
  const modelChart = screen.getByTestId("overview-model-chart");
  expect(timeChart.parentElement).toHaveClass("flex-1");
  expect(modelChart.parentElement).toHaveClass("flex-1");
  expect(timeChart.parentElement).not.toHaveClass("h-80");
  expect(modelChart.parentElement).not.toHaveClass("h-80");
  expect(screen.getByRole("img", { name: /模型调用占比/ })).toBeInTheDocument();
  expect(screen.getByText("gpt-test")).toBeInTheDocument();
  expect(String(fetchMock.mock.calls[0]?.[0])).toContain("range=1h");
  expect(rendered.container.querySelector(".rounded-\\[14px\\]")).toBeNull();
  const rangeIndicator = rendered.container.querySelector("[data-sliding-selection-indicator]");
  expect(rangeIndicator).toHaveAttribute("data-active-value", "1h");

  fireEvent.click(screen.getByRole("button", { name: "7 天" }));
  expect(rangeIndicator).toBeInTheDocument();
  expect(rangeIndicator).toHaveAttribute("data-active-value", "7d");
  await waitFor(() => expect(screen.getByTestId("location")).toHaveTextContent("range=7d"));
  await waitFor(() =>
    expect(fetchMock.mock.calls.some((call) => String(call[0]).includes("range=7d"))).toBe(true),
  );
  expect(await screen.findByText("0.0002")).toBeInTheDocument();
  expect(await screen.findByRole("group", { name: "按时间展示的 28 个调用桶" })).toBeInTheDocument();
});

test("limits the compact model pie to eight slices", () => {
  const models: OverviewUsageModel[] = Array.from({ length: 9 }, (_, index) => ({
    publicModel: `model-${index + 1}`,
    isOther: false,
    requestCount: 1,
    successfulRequestCount: 1,
    failedRequestCount: 0,
    tokenUsageRequestCount: 1,
    inputTokens: 1n,
    outputTokens: 0n,
    totalTokens: 1n,
  }));

  render(<OverviewModelChart models={models} />);

  const legend = screen.getByRole("list", { name: "模型调用占比图例" });
  expect(within(legend).getAllByRole("listitem")).toHaveLength(8);
  expect(within(legend).getByText("其余 2 个模型")).toBeInTheDocument();
  expect(legend.querySelectorAll(":scope > li > div")).toHaveLength(8);
});

test("centers the empty model state in the shared chart height", () => {
  render(<OverviewModelChart models={[]} />);

  expect(screen.getByText("当前时间段还没有模型调用。")).toHaveClass("h-full");
});

test("does not invent a remaining model count for an API aggregate", () => {
  const models: OverviewUsageModel[] = Array.from({ length: 8 }, (_, index) => ({
    publicModel: `model-${index + 1}`,
    isOther: false,
    requestCount: 1,
    successfulRequestCount: 1,
    failedRequestCount: 0,
    tokenUsageRequestCount: 1,
    inputTokens: 1n,
    outputTokens: 0n,
    totalTokens: 1n,
  }));
  models.push({
    publicModel: null,
    isOther: true,
    requestCount: 3,
    successfulRequestCount: 3,
    failedRequestCount: 0,
    tokenUsageRequestCount: 3,
    inputTokens: 3n,
    outputTokens: 0n,
    totalTokens: 3n,
  });

  render(<OverviewModelChart models={models} />);

  const legend = screen.getByRole("list", { name: "模型调用占比图例" });
  expect(within(legend).getByText("其余模型")).toBeInTheDocument();
  expect(within(legend).queryByText(/其余 \d+ 个模型/)).not.toBeInTheDocument();
});

function LocationProbe() {
  const location = useLocation();
  return <output data-testid="location">{location.search}</output>;
}
