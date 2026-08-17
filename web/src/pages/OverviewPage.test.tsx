import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

vi.mock("@/features/overview-usage", () => ({
  OverviewUsageSection: () => <section data-testid="usage">调用分析</section>,
}));
vi.mock("@/features/system-status", () => ({
  SystemOverview: () => <section data-testid="system">系统状态</section>,
  ProviderLoadSummary: () => <section data-testid="providers">Provider 负载</section>,
}));

import { OverviewPage } from "./OverviewPage";

test("uses the shared page width and puts provider load after call analysis", () => {
  render(<OverviewPage />);

  const page = screen.getByTestId("system").parentElement;
  expect(page).not.toHaveClass("mx-auto", "max-w-[1440px]");
  expect(Array.from(page?.children ?? []).map((child) => child.textContent)).toEqual([
    "系统状态",
    "调用分析",
    "Provider 负载",
  ]);
});
