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

test("aligns expanded desktop icons with the sidebar toggle", () => {
  render(
    <MemoryRouter>
      <AppNavigation />
    </MemoryRouter>,
  );

  expect(screen.getByRole("link", { name: "系统总览" })).toHaveClass("pl-4", "pr-3");
});
