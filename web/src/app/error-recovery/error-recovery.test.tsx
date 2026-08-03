import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { AppErrorBoundary } from "./AppErrorBoundary";
import { PageLoadingFallback } from "./PageLoadingFallback";
import { appRoutes } from "@/app/router";

afterEach(() => {
  vi.restoreAllMocks();
});

test("catches rendering failures outside the router and offers a reload", () => {
  vi.spyOn(console, "error").mockImplementation(() => undefined);

  render(
    <AppErrorBoundary>
      <ThrowingView />
    </AppErrorBoundary>,
  );

  expect(screen.getByRole("heading", { name: "管理界面发生错误" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "重新加载" })).toHaveAttribute(
    "href",
    window.location.href,
  );
});

test("the production root route replaces router failures with the Chinese recovery page", async () => {
  vi.spyOn(console, "error").mockImplementation(() => undefined);
  const errorElement = appRoutes[0]?.errorElement;
  expect(errorElement).toBeTruthy();
  const router = createMemoryRouter([
    {
      path: "/",
      element: <ThrowingView />,
      errorElement,
    },
  ]);

  render(<RouterProvider router={router} />);

  expect(await screen.findByRole("heading", { name: "页面加载失败" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "重新加载" })).toBeInTheDocument();
});

test("lazy pages expose a visible and accessible loading state", () => {
  render(<PageLoadingFallback />);

  expect(screen.getByRole("status", { name: "正在加载页面" })).toBeInTheDocument();
  expect(screen.getByText("正在加载页面")).toBeVisible();
});

function ThrowingView(): never {
  throw new Error("render failed");
}
