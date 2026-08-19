import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { OAuthQuotaEstimate } from "../api/oauth-quota-contracts";
import { QuotaEstimate } from "./OAuthQuotaEstimate";

test("shows that no local statistics exist before local usage is recorded", () => {
  render(<QuotaEstimate rateCard={rateCard} estimate={estimate({
    estimatedCapacityCredits: null,
    estimatedUsedCredits: null,
    estimatedRemainingCredits: null,
  })} />);

  const trigger = screen.getByText("暂无");
  fireEvent.focus(trigger);
  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toHaveTextContent("暂无");
  expect(trigger).toHaveAttribute("aria-describedby");
});

test("shows direct local usage while capacity is waiting for enough official usage", () => {
  render(<QuotaEstimate rateCard={rateCard} estimate={estimate({
    estimatedCapacityCredits: null,
    estimatedUsedCredits: 10,
    estimatedRemainingCredits: null,
  })} />);

  const value = screen.getByText("$0.40/暂无");
  expect(value).toHaveAttribute("aria-label", "本地额度统计：已用 $0.40，总量 暂无");
  fireEvent.mouseEnter(value);
  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toHaveTextContent("本地已用 $0.40 · 总量暂无");
  expect(tooltip).toHaveTextContent("当前官方周期 RequestLog 直接总和");
  expect(tooltip).toHaveTextContent("整周期可比、官方使用率至少 2% 且本地已用为正");
});

test("shows pending capacity in Credits when no rate card exists", () => {
  render(<QuotaEstimate rateCard={null} estimate={estimate({
    estimatedCapacityCredits: null,
    estimatedUsedCredits: 10,
    estimatedRemainingCredits: null,
  })} />);

  expect(screen.getByText("10 Credits/暂无")).toBeInTheDocument();
});

test("statistics expose the stable capacity calculation on hover", () => {
  render(<QuotaEstimate estimate={base} rateCard={rateCard} />);

  fireEvent.mouseEnter(screen.getByText("$0.40/$1.00"));
  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toHaveTextContent("剩余 $0.60");
  expect(tooltip).toHaveTextContent("25 Credits = $1");
  expect(tooltip).toHaveTextContent("当前官方周期 RequestLog 直接总和");
  expect(tooltip).toHaveTextContent("比例推算至整周期");
});

const base: OAuthQuotaEstimate = {
  windowId: "primary",
  windowKind: "time",
  limitWindowSeconds: 18_000,
  windowResetAt: 1_900_000_300,
  estimatedCapacityCredits: 25,
  estimatedUsedCredits: 10,
  estimatedRemainingCredits: 15,
};

const rateCard = {
  id: "openai_codex_credits_2026_08_11",
  creditsPerUsd: 25,
};

function estimate(overrides: Partial<OAuthQuotaEstimate>): OAuthQuotaEstimate {
  return { ...base, ...overrides };
}
