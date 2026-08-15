import { render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { OAuthAccountCard } from "./OAuthAccountCard";

test("keeps the latest quota timestamp and renders model expiry metrics", () => {
  render(
    <OAuthAccountCard
      presentation={{
        id: "account-1",
        title: "Primary Codex",
        subtitle: "owner@example.com",
        enabled: true,
        badges: [{ key: "runtime-status", label: "过期", tone: "danger" }],
        metrics: [
          { key: "models", label: "模型", value: "8" },
          { key: "expires", label: "过期", value: "2026/8/23 14:03" },
        ],
        modelCatalog: ["gpt-5.6-sol"],
      }}
      proxyLabel="跟随全局（DIRECT 本机直连）"
      pending={false}
      lastUpdatedAt={1_900_000_000}
      onToggleEnabled={vi.fn()}
      onViewModels={vi.fn()}
      onEdit={vi.fn()}
      onDelete={vi.fn()}
    />,
  );

  expect(screen.getByText("8")).toBeInTheDocument();
  expect(screen.getByText("2026/8/23 14:03").parentElement)
    .toHaveTextContent("过期2026/8/23 14:03");
  const expiredBadge = screen.getByLabelText("账号状态：过期");
  expect(expiredBadge).toHaveClass(
    "bg-danger/10",
    "text-danger",
  );
  expect(expiredBadge.closest("[data-floating-bounds]")).toHaveClass(
    "border-danger/20",
    "bg-linear-to-b",
    "from-danger/10",
    "via-danger/[0.035]",
    "to-surface",
  );
  expect(screen.queryByText("状态")).not.toBeInTheDocument();
  const updatedAt = screen.getByText(/最后更新 \d{2}\/\d{2} \d{2}:\d{2}:\d{2}/);
  expect(within(updatedAt.parentElement!).getByRole("button", {
    name: "编辑 Primary Codex",
  })).toBeInTheDocument();
});

test("gives an exhausted account a warning gradient", () => {
  render(
    <OAuthAccountCard
      presentation={{
        id: "account-1",
        title: "Primary Codex",
        subtitle: "owner@example.com",
        enabled: true,
        badges: [{ key: "quota-exhausted", label: "耗尽", tone: "warning" }],
        metrics: [],
        modelCatalog: [],
      }}
      proxyLabel="跟随全局（SOCKS5 US）"
      pending={false}
      onToggleEnabled={vi.fn()}
      onViewModels={vi.fn()}
      onEdit={vi.fn()}
      onDelete={vi.fn()}
    />,
  );

  const exhaustedBadge = screen.getByLabelText("账号状态：耗尽");
  expect(exhaustedBadge).toHaveClass("bg-warning/12", "text-warning");
  expect(exhaustedBadge.closest("[data-floating-bounds]")).toHaveClass(
    "border-warning/20",
    "bg-linear-to-b",
    "from-warning/10",
    "via-warning/[0.035]",
    "to-surface",
  );
});

test("gives a healthy account a success gradient", () => {
  render(
    <OAuthAccountCard
      presentation={{
        id: "account-1",
        title: "Primary Codex",
        subtitle: "owner@example.com",
        enabled: true,
        badges: [{ key: "runtime-status", label: "正常", tone: "success" }],
        metrics: [],
        modelCatalog: [],
      }}
      proxyLabel="跟随全局（SOCKS5 US）"
      pending={false}
      onToggleEnabled={vi.fn()}
      onViewModels={vi.fn()}
      onEdit={vi.fn()}
      onDelete={vi.fn()}
    />,
  );

  const healthyBadge = screen.getByLabelText("账号状态：正常");
  expect(healthyBadge).toHaveClass("bg-success/10", "text-success");
  expect(healthyBadge.closest("[data-floating-bounds]")).toHaveClass(
    "border-success/20",
    "bg-linear-to-b",
    "from-success/10",
    "via-success/[0.035]",
    "to-surface",
  );
});

test.each([
  ["free", "border-subtle", "bg-surface-muted", "text-tertiary"],
  ["Go", "border-plan-entry/20", "bg-plan-entry/10", "text-plan-entry"],
  ["plus", "border-accent/15", "bg-accent/10", "text-accent-copy"],
  ["Pro", "border-plan-pro/20", "bg-plan-pro/10", "text-plan-pro"],
  ["Max 5x", "border-plan-pro/20", "bg-plan-pro/10", "text-plan-pro"],
  ["Max 20x", "border-plan-premium/20", "bg-plan-premium/12", "text-plan-premium"],
  ["SuperGrok Heavy", "border-plan-premium/20", "bg-plan-premium/12", "text-plan-premium"],
  ["team", "border-success/20", "bg-success/10", "text-success"],
  ["Business", "border-success/20", "bg-success/10", "text-success"],
  ["Enterprise", "border-plan-institution/20", "bg-plan-institution/10", "text-plan-institution"],
  ["k12", "border-plan-institution/20", "bg-plan-institution/10", "text-plan-institution"],
])("gives the %s plan a distinct restrained tier color", (plan, border, background, text) => {
  render(
    <OAuthAccountCard
      presentation={{
        id: "account-1",
        title: "Primary Codex",
        subtitle: "owner@example.com",
        enabled: true,
        badges: [{ key: "plan", label: plan, tone: "neutral" }],
        metrics: [],
        modelCatalog: [],
      }}
      proxyLabel="跟随全局（SOCKS5 TW）"
      pending={false}
      onToggleEnabled={vi.fn()}
      onViewModels={vi.fn()}
      onEdit={vi.fn()}
      onDelete={vi.fn()}
    />,
  );

  expect(screen.getByLabelText(`账号套餐：${plan}`)).toHaveClass(
    border,
    background,
    text,
  );
});

test("gives an unknown official plan a stable non-neutral color", () => {
  const renderUnknownPlan = () => render(
    <OAuthAccountCard
      presentation={{
        id: "account-1",
        title: "Primary Codex",
        subtitle: "owner@example.com",
        enabled: true,
        badges: [{ key: "plan", label: "Future Ultra", tone: "neutral" }],
        metrics: [],
        modelCatalog: [],
      }}
      proxyLabel="跟随全局（SOCKS5 TW）"
      pending={false}
      onToggleEnabled={vi.fn()}
      onViewModels={vi.fn()}
      onEdit={vi.fn()}
      onDelete={vi.fn()}
    />,
  );
  const first = renderUnknownPlan();
  const firstClassName = screen.getByLabelText("账号套餐：Future Ultra").className;
  first.unmount();
  renderUnknownPlan();

  const secondBadge = screen.getByLabelText("账号套餐：Future Ultra");
  expect(secondBadge.className).toBe(firstClassName);
  expect(secondBadge).not.toHaveClass("text-tertiary");
});
