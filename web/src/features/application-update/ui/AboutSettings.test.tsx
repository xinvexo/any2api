import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { AboutSettings } from "./AboutSettings";

afterEach(() => vi.restoreAllMocks());

test("checks and installs a newer official release", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/update/check") && init?.method === "POST") {
      return jsonResponse({
        current_version: "1.0.0",
        latest_version: "1.1.0",
        update_available: true,
        release_url: "https://github.com/xinvexo/any2api/releases/tag/v1.1.0",
        published_at: "2026-07-29T00:00:00Z",
      });
    }
    if (path.endsWith("/update/install") && init?.method === "POST") {
      return jsonResponse({ installed_version: "1.1.0", restart_requested: true });
    }
    return jsonResponse(about());
  });
  renderAbout();

  expect(await screen.findByText("v1.0.0")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: /xinvexo\/any2api/ })).toHaveAttribute(
    "href",
    "https://github.com/xinvexo/any2api",
  );
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("发现新版本 v1.1.0")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "更新到 v1.1.0" }));
  expect(await screen.findByText("v1.1.0 已安装，服务正在优雅重启。")).toBeInTheDocument();
  await waitFor(() => {
    expect(fetchMock.mock.calls.some(([input]) => String(input).endsWith("/update/install"))).toBe(true);
  });
});

test("only reports an unsupported environment after update is requested", async () => {
  const fetchMock = vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/update/check") && init?.method === "POST") {
      return jsonResponse({
        current_version: "1.0.0",
        latest_version: "1.1.0",
        update_available: true,
        release_url: "https://github.com/xinvexo/any2api/releases/tag/v1.1.0",
        published_at: null,
      });
    }
    if (path.endsWith("/update/install") && init?.method === "POST") {
      return new Response(
        JSON.stringify({
          error: {
            code: "update_unsupported",
            message: "this build cannot replace itself",
          },
        }),
        { status: 409, headers: { "Content-Type": "application/json" } },
      );
    }
    return jsonResponse(about());
  });
  renderAbout();

  expect(await screen.findByText("v1.0.0")).toBeInTheDocument();
  expect(screen.queryByText(/不支持/)).not.toBeInTheDocument();
  expect(screen.queryByText(/原地更新/)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("发现新版本 v1.1.0")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "更新到 v1.1.0" }));
  expect(await screen.findByText("当前运行环境不支持自动更新。")).toBeInTheDocument();
  expect(
    fetchMock.mock.calls.some(([input]) => String(input).endsWith("/update/install")),
  ).toBe(true);
});

function renderAbout() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <AboutSettings />
    </QueryClientProvider>,
  );
}

function about() {
  return {
    current_version: "1.0.0",
    repository_url: "https://github.com/xinvexo/any2api",
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
