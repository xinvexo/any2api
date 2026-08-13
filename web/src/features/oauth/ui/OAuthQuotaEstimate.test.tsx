import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { OAuthQuotaEstimate } from "../api/oauth-quota-contracts";
import { QuotaEstimate } from "./OAuthQuotaEstimate";

test("shows that no local statistics exist before the first interval", () => {
  render(<QuotaEstimate rateCard={rateCard} estimate={estimate({
    estimatedCapacityCredits: null,
    estimatedUsedCredits: null,
    estimatedRemainingCredits: null,
    completedIntervalCount: 0,
  })} />);

  const trigger = screen.getByText("尚无本地统计");
  fireEvent.focus(trigger);
  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toHaveTextContent("尚无本地统计");
  expect(trigger).toHaveAttribute("aria-describedby");
});

test("statistics expose the detailed calculation on hover", () => {
  render(<QuotaEstimate estimate={base} rateCard={rateCard} />);

  fireEvent.mouseEnter(screen.getByText("$0.40/$1.00"));
  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toHaveTextContent("剩余 $0.60");
  expect(tooltip).toHaveTextContent("累计区间 3");
  expect(tooltip).toHaveTextContent("25 Credits = $1");
});

test("a reset boundary keeps all prior intervals in the cumulative count", () => {
  render(<QuotaEstimate rateCard={rateCard} estimate={base} />);

  const value = screen.getByText("$0.40/$1.00");
  fireEvent.mouseEnter(value);
  expect(screen.getByRole("tooltip")).toHaveTextContent("累计区间 3");
});

const base: OAuthQuotaEstimate = {
  windowId: "primary",
  windowKind: "time",
  limitWindowSeconds: 18_000,
  windowResetAt: 1_900_000_300,
  estimatedCapacityCredits: 25,
  estimatedUsedCredits: 10,
  estimatedRemainingCredits: 15,
  completedIntervalCount: 3,
};

const rateCard = {
  id: "openai_codex_credits_2026_08_11",
  creditsPerUsd: 25,
};

function estimate(overrides: Partial<OAuthQuotaEstimate>): OAuthQuotaEstimate {
  return { ...base, ...overrides };
}
