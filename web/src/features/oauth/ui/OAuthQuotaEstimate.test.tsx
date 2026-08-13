import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { OAuthQuotaEstimate } from "../api/oauth-quota-contracts";
import { QuotaEstimate } from "./OAuthQuotaEstimate";

test("cold start stays visibly unknown until an interval sample exists", () => {
  render(<QuotaEstimate rateCard={rateCard} estimate={estimate({
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

  const trigger = screen.getByText("容量校准中");
  fireEvent.focus(trigger);
  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toHaveTextContent("容量尚未形成");
  expect(tooltip).not.toHaveTextContent("不是概率");
  expect(trigger).toHaveAttribute("aria-describedby");
});

test("degraded estimates remain available but are marked approximate", () => {
  render(<QuotaEstimate rateCard={rateCard} estimate={estimate({
    confidence: "degraded",
    latestInterval: {
      ...base.latestInterval,
      status: "telemetry_incomplete",
      queueDroppedRequestLogs: 1,
    },
  })} />);

  const value = screen.getByText("≈$0.40/$1.00");
  fireEvent.mouseEnter(value);
  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toHaveTextContent("本地遥测不完整");
  expect(tooltip).toHaveTextContent("证据状态 不完整");
  expect(tooltip).toHaveTextContent("遥测缺口：队列丢失 1");
  expect(tooltip).not.toHaveTextContent("样本相对 MAD");
  expect(tooltip).not.toHaveTextContent("Epoch");
  expect(tooltip).not.toHaveTextContent("非上游余额");
  expect(tooltip).not.toHaveTextContent("现有估算仅供参考");
});

test("stable estimates expose the detailed calculation on hover", () => {
  render(<QuotaEstimate estimate={base} rateCard={rateCard} />);

  fireEvent.mouseEnter(screen.getByText("$0.40/$1.00"));
  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toHaveTextContent("剩余 $0.60");
  expect(tooltip).toHaveTextContent(
    "区间本地消费 0.25 Credits · 官方使用率变化 1%",
  );
  expect(tooltip).toHaveTextContent("证据状态 稳定");
  expect(tooltip).toHaveTextContent("25 Credits = $1");
  expect(tooltip).not.toHaveTextContent("仅展示换算");
  expect(tooltip).not.toHaveTextContent("费率卡");
});

test("interval-reaching prune is reported as a telemetry gap", () => {
  render(<QuotaEstimate rateCard={rateCard} estimate={estimate({
    confidence: "degraded",
    latestInterval: {
      ...base.latestInterval,
      status: "telemetry_incomplete",
      intervalPruned: true,
    },
  })} />);

  const value = screen.getByText("≈$0.40/$1.00");
  fireEvent.mouseEnter(value);
  expect(screen.getByRole("tooltip")).toHaveTextContent(
    "遥测缺口：日志清理删除了区间数据",
  );
});

test("a rollover prior keeps the estimate and shows per-epoch sample counts", () => {
  render(<QuotaEstimate rateCard={rateCard} estimate={estimate({
    confidence: "learning",
    freshSampleCount: 0,
    latestInterval: {
      ...base.latestInterval,
      status: "accumulating",
    },
  })} />);

  const value = screen.getByText("$0.40/$1.00");
  fireEvent.mouseEnter(value);
  const tooltip = screen.getByRole("tooltip");
  expect(tooltip).toHaveTextContent("证据状态 容量校准中");
  expect(tooltip).toHaveTextContent("累计观测中");
  expect(tooltip).toHaveTextContent("容量样本 3 · 本窗口期 0");
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
  freshSampleCount: 2,
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
    intervalPruned: false,
  },
  rateCards: ["openai_codex_credits_2026_08_11"],
};

const rateCard = {
  id: "openai_codex_credits_2026_08_11",
  creditsPerUsd: 25,
};

function estimate(overrides: Partial<OAuthQuotaEstimate>): OAuthQuotaEstimate {
  return { ...base, ...overrides };
}
