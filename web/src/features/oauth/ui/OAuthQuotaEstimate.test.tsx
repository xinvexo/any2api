import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { OAuthQuotaEstimate } from "../api/oauth-quota-contracts";
import { QuotaEstimate } from "./OAuthQuotaEstimate";

test("cold start stays visibly unknown until an interval sample exists", () => {
  render(<QuotaEstimate estimate={estimate({
    confidence: "unknown",
    estimatedCapacityCredits: null,
    estimatedUsedCredits: null,
    estimatedRemainingCredits: null,
    sampleCount: 0,
    latestInterval: {
      ...base.latestInterval,
      status: "awaiting_baseline",
    },
  })} />);

  const trigger = screen.getByText("学习中");
  fireEvent.focus(trigger);
  expect(screen.getByRole("tooltip")).toHaveTextContent("需要两个可靠官方快照");
  expect(trigger).toHaveAttribute("aria-describedby");
});

test("degraded estimates remain available but are marked approximate", () => {
  render(<QuotaEstimate estimate={estimate({
    confidence: "degraded",
    latestInterval: {
      ...base.latestInterval,
      status: "external_usage_suspected",
    },
  })} />);

  const value = screen.getByText("≈$0.40/$1.00");
  fireEvent.mouseEnter(value);
  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toHaveTextContent("疑似外部消费");
  expect(tooltip).toHaveTextContent("置信度 已降级");
  expect(tooltip).not.toHaveTextContent("样本相对 MAD");
  expect(tooltip).not.toHaveTextContent("Epoch");
  expect(tooltip).not.toHaveTextContent("非上游余额");
});

const base: OAuthQuotaEstimate = {
  windowId: "primary",
  windowKind: "time",
  limitWindowSeconds: 18_000,
  windowResetAt: 1_900_000_300,
  epoch: 1,
  epochStartedAt: 1_899_999_000,
  confidence: "stable",
  estimatedCapacityCredits: 25,
  estimatedUsedCredits: 10,
  estimatedRemainingCredits: 15,
  sampleCount: 3,
  relativeMad: 0.01,
  latestInterval: {
    status: "valid_sample",
    startedAt: 1_899_999_700,
    endedAt: 1_900_000_000,
    deltaUsedPercent: 1,
    localCostCredits: 0.25,
    unpricedRequestCount: 0,
    queueDroppedRequestLogs: 0,
    storageFailedRequestLogs: 0,
    prunedRequestLogs: 0,
  },
  rateCards: ["openai_codex_credits_2026_08_11"],
};

function estimate(overrides: Partial<OAuthQuotaEstimate>): OAuthQuotaEstimate {
  return { ...base, ...overrides };
}
