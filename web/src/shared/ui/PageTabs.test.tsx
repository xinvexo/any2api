import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { expect, test } from "vitest";

import { PageTabs } from "./PageTabs";

const items = [
  { label: "基础", path: "/settings/basic" },
  { label: "路由策略", path: "/settings/routing" },
  { label: "关于", path: "/settings/about" },
] as const;

test("slides one background across variable-width route tabs", () => {
  const { container } = render(
    <MemoryRouter initialEntries={["/settings/basic"]}>
      <PageTabs items={items} ariaLabel="系统设置分类" />
    </MemoryRouter>,
  );
  const indicator = container.querySelector("[data-sliding-selection-indicator]");
  const routing = screen.getByRole("link", { name: "路由策略" });

  expect(indicator).toHaveAttribute("data-active-value", "/settings/basic");
  expect(routing.className).not.toContain("hover:bg-");

  fireEvent.click(routing);

  expect(indicator).toHaveAttribute("data-active-value", "/settings/routing");
  expect(routing).toHaveAttribute("aria-current", "page");
});
