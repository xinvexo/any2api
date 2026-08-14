import { render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { OAuthAccountCard } from "./OAuthAccountCard";

test("keeps the latest quota timestamp in the action row and colors runtime status", () => {
  render(
    <OAuthAccountCard
      presentation={{
        id: "account-1",
        title: "Primary Codex",
        subtitle: "owner@example.com",
        enabled: true,
        badges: [],
        metrics: [{
          key: "runtime-status",
          label: "状态",
          value: "正常",
          tone: "success",
        }],
        modelCatalog: ["gpt-5.6-sol"],
      }}
      proxyLabel="跟随全局 · DIRECT"
      pending={false}
      lastUpdatedAt={1_900_000_000}
      onToggleEnabled={vi.fn()}
      onViewModels={vi.fn()}
      onEdit={vi.fn()}
      onDelete={vi.fn()}
    />,
  );

  expect(screen.getByText("正常")).toHaveClass("text-success");
  const updatedAt = screen.getByText(/最后更新 \d{2}\/\d{2} \d{2}:\d{2}:\d{2}/);
  expect(within(updatedAt.parentElement!).getByRole("button", {
    name: "编辑 Primary Codex",
  })).toBeInTheDocument();
});
