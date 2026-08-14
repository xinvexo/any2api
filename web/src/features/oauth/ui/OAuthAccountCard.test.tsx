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
  expect(screen.getByLabelText("账号状态：过期")).toHaveClass(
    "bg-danger/10",
    "text-danger",
  );
  expect(screen.queryByText("状态")).not.toBeInTheDocument();
  const updatedAt = screen.getByText(/最后更新 \d{2}\/\d{2} \d{2}:\d{2}:\d{2}/);
  expect(within(updatedAt.parentElement!).getByRole("button", {
    name: "编辑 Primary Codex",
  })).toBeInTheDocument();
});
