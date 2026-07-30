import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { expect, test } from "vitest";

import { AppNavigation } from "./AppNavigation";

test("uses the compact accent treatment for mobile navigation", () => {
  render(
    <MemoryRouter initialEntries={["/logs"]}>
      <AppNavigation variant="mobile" />
    </MemoryRouter>,
  );

  const activeLink = screen.getByRole("link", { name: "请求日志" });
  const inactiveLink = screen.getByRole("link", { name: "系统总览" });

  expect(activeLink).toHaveClass("h-10", "rounded-[8px]", "bg-accent/10", "text-accent");
  expect(inactiveLink).toHaveClass("text-primary");
  expect(inactiveLink).not.toHaveClass("bg-accent/10");
});

test("keeps desktop icons on the same rail while the sidebar collapses", () => {
  const rendered = render(
    <MemoryRouter>
      <AppNavigation />
    </MemoryRouter>,
  );

  const expandedLink = screen.getByRole("link", { name: "系统总览" });
  expect(expandedLink).toHaveClass("pl-4", "pr-3");
  expect(expandedLink.querySelector("svg")).toHaveClass("shrink-0");
  expect(expandedLink.querySelector("span")).toHaveClass(
    "min-w-0",
    "overflow-hidden",
    "whitespace-nowrap",
  );

  rendered.rerender(
    <MemoryRouter>
      <AppNavigation collapsed />
    </MemoryRouter>,
  );

  expect(screen.getByRole("link", { name: "系统总览" })).toHaveClass("px-4");
  expect(screen.getByRole("link", { name: "系统总览" })).not.toHaveClass("justify-center");
});
