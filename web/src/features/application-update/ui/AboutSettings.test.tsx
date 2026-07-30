import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { ApplicationUpdateProvider } from "../model/ApplicationUpdateProvider";
import { AboutSettings } from "./AboutSettings";

const { reloadApplicationMock } = vi.hoisted(() => ({
  reloadApplicationMock: vi.fn(),
}));

vi.mock("../model/reload-application", () => ({
  reloadApplication: reloadApplicationMock,
}));

afterEach(() => {
  vi.restoreAllMocks();
  reloadApplicationMock.mockReset();
  window.sessionStorage.clear();
  document.getElementById("root")?.remove();
});

test("locks the page, shows progress, and reloads after the target build is healthy", async () => {
  let phase = "downloading";
  let healthVersion = "1.0.0";
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/update/check") && init?.method === "POST") {
      return jsonResponse(updateCheck());
    }
    if (path.endsWith("/update/install") && init?.method === "POST") {
      return jsonResponse(updateStatus({ phase: "checking" }), 202);
    }
    if (path.endsWith("/update/status")) {
      return jsonResponse(phase === "downloading"
        ? updateStatus({
            phase,
            target_version: "1.1.0",
            downloaded_bytes: 512,
            total_bytes: 1024,
          })
        : updateStatus({ phase, target_version: "1.1.0" }));
    }
    if (path.endsWith("/api/health")) {
      return jsonResponse({ application_version: healthVersion });
    }
    return jsonResponse(about());
  });
  renderAbout();

  expect(await screen.findByText("v1.0.0")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("发现新版本 v1.1.0")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "更新到 v1.1.0" }));

  expect(await screen.findByRole("dialog", { name: "正在下载 v1.1.0" })).toBeInTheDocument();
  expect(screen.getByRole("progressbar", { name: "更新下载进度" })).toHaveAttribute(
    "aria-valuenow",
    "50",
  );
  expect(screen.queryByRole("button", { name: "返回" })).not.toBeInTheDocument();
  expect(document.getElementById("root")).toHaveAttribute("inert");
  const unload = new Event("beforeunload", { cancelable: true });
  window.dispatchEvent(unload);
  expect(unload.defaultPrevented).toBe(true);

  phase = "installing";
  expect(await screen.findByRole("dialog", { name: "正在安装" })).toBeInTheDocument();
  phase = "restarting";
  expect(await screen.findByRole("dialog", { name: "正在重新启动" })).toBeInTheDocument();
  healthVersion = "1.1.0";
  expect(await screen.findByRole("dialog", { name: "更新完成" })).toBeInTheDocument();
  await waitFor(() => expect(reloadApplicationMock).toHaveBeenCalledOnce(), { timeout: 2_000 });
});

test.each([
  {
    name: "another client already started the task",
    start: () => Promise.resolve(new Response(
      JSON.stringify({
        error: {
          code: "update_in_progress",
          message: "an application update is already in progress",
        },
      }),
      { status: 409, headers: { "Content-Type": "application/json" } },
    )),
  },
  {
    name: "the accepted install response was lost",
    start: () => Promise.reject(new TypeError("connection closed")),
  },
])("keeps the page locked when $name", async ({ start }) => {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/update/check") && init?.method === "POST") {
      return jsonResponse(updateCheck());
    }
    if (path.endsWith("/update/install") && init?.method === "POST") {
      return start();
    }
    if (path.endsWith("/update/status")) {
      return jsonResponse(updateStatus({
        phase: "downloading",
        target_version: "1.1.0",
        downloaded_bytes: 512,
        total_bytes: 1024,
      }));
    }
    if (path.endsWith("/api/health")) {
      return jsonResponse({ application_version: "1.0.0" });
    }
    return jsonResponse(about());
  });
  renderAbout();

  fireEvent.click(await screen.findByRole("button", { name: "检查更新" }));
  fireEvent.click(await screen.findByRole("button", { name: "更新到 v1.1.0" }));

  expect(await screen.findByRole("dialog", { name: "正在下载 v1.1.0" })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "返回" })).not.toBeInTheDocument();
  expect(document.getElementById("root")).toHaveAttribute("inert");
});

test("only unlocks the full-screen flow after the update itself fails", async () => {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input, init) => {
    const path = String(input);
    if (path.endsWith("/update/check") && init?.method === "POST") {
      return jsonResponse(updateCheck());
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

  fireEvent.click(await screen.findByRole("button", { name: "检查更新" }));
  fireEvent.click(await screen.findByRole("button", { name: "更新到 v1.1.0" }));

  expect(await screen.findByRole("dialog", { name: "更新未完成" })).toBeInTheDocument();
  expect(screen.getByText("当前运行环境不支持自动更新。")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "重新尝试" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "返回" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(document.getElementById("root")).not.toHaveAttribute("inert");
});

function renderAbout() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const container = document.createElement("div");
  container.id = "root";
  document.body.append(container);
  return render(
    <QueryClientProvider client={client}>
      <ApplicationUpdateProvider>
        <AboutSettings />
      </ApplicationUpdateProvider>
    </QueryClientProvider>,
    { container },
  );
}

function about() {
  return {
    current_version: "1.0.0",
    repository_url: "https://github.com/xinvexo/any2api",
  };
}

function updateCheck() {
  return {
    current_version: "1.0.0",
    latest_version: "1.1.0",
    update_available: true,
    release_url: "https://github.com/xinvexo/any2api/releases/tag/v1.1.0",
    published_at: "2026-07-29T00:00:00Z",
  };
}

function updateStatus(overrides: Record<string, unknown>) {
  return {
    phase: "idle",
    target_version: null,
    downloaded_bytes: null,
    total_bytes: null,
    failure_code: null,
    ...overrides,
  };
}

function jsonResponse(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}
