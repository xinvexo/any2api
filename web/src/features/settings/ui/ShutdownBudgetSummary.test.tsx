import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type { SettingItem } from "../api/settings-contracts";
import type { SettingDraft } from "../model/setting-draft";
import { ShutdownBudgetSummary } from "./ShutdownBudgetSummary";

const items = [
  durationSetting("shutdown.request_grace_period", 30, 300),
  durationSetting("shutdown.finalize_timeout", 5, 60),
];

test("shows default and maximum cumulative shutdown budgets from current drafts", () => {
  const { rerender } = renderSummary({
    "shutdown.request_grace_period": "30",
    "shutdown.finalize_timeout": "5",
  });

  const summary = screen.getByRole("status", { name: "优雅停机累计等待预算" });
  expect(summary).toHaveTextContent("最长 1 分钟");
  expect(summary).toHaveTextContent("30 秒请求宽限 + 6 × 5 秒单阶段收尾");
  expect(summary).toHaveTextContent("每个收尾阶段分别计时");

  rerender(summaryElement({
    "shutdown.request_grace_period": "300",
    "shutdown.finalize_timeout": "60",
  }));

  expect(summary).toHaveTextContent("最长 11 分钟");
  expect(summary).toHaveTextContent("300 秒请求宽限 + 6 × 60 秒单阶段收尾");
});

test("does not show a misleading total for an invalid shutdown draft", () => {
  renderSummary({
    "shutdown.request_grace_period": "invalid",
    "shutdown.finalize_timeout": "60",
  });

  const summary = screen.getByRole("status", { name: "优雅停机累计等待预算" });
  expect(summary).toHaveTextContent("修正停机时间后即可计算累计等待预算");
  expect(summary).not.toHaveTextContent("最长");
});

function renderSummary(drafts: Record<string, SettingDraft>) {
  return render(summaryElement(drafts));
}

function summaryElement(drafts: Record<string, SettingDraft>) {
  return (
    <ShutdownBudgetSummary
      items={items}
      draftFor={(item) => drafts[item.key] ?? ""}
    />
  );
}

function durationSetting(key: string, defaultValue: number, maxValue: number): SettingItem {
  return {
    key,
    valueType: "duration_secs",
    defaultValue,
    overrideValue: null,
    effectiveValue: defaultValue,
    minValue: 1,
    maxValue,
    allowedValues: null,
    options: null,
    applyMode: "hot_reload",
    webGroup: "优雅停机",
    description: "Test setting",
  };
}
