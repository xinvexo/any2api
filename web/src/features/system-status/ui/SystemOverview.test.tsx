import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { SystemOverview } from "./SystemOverview";
import { clearNotifications, getNotifications } from "@/shared/notifications";

function Wrapper({ children }: PropsWithChildren) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <MemoryRouter>
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    </MemoryRouter>
  );
}

afterEach(() => {
  clearNotifications();
  vi.restoreAllMocks();
});

test("renders session and live task metrics at the top", async () => {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = String(input);
    if (url.includes("/api/admin/affinity")) {
      return jsonResponse({
        config_revision: 7,
        affinity_enabled: true,
        active_session_count: 3,
        creating_session_count: 1,
      });
    }
    return jsonResponse({
      status: "ok",
      config_revision: 7,
      scheduler_epoch: 2,
      shutdown_phase: "running",
      active_requests: 1,
      background_tasks: 2,
    });
  });

  const rendered = render(<SystemOverview />, { wrapper: Wrapper });

  expect(await screen.findByText("运行正常")).toBeInTheDocument();
  expect(screen.getByText("活动显式会话")).toBeInTheDocument();
  expect(screen.getByText("3")).toBeInTheDocument();
  expect(screen.getByText("建立中显式会话")).toBeInTheDocument();
  expect(screen.getByText("1")).toBeInTheDocument();
  expect(screen.getByText(/不含 Response ID 续接/)).toBeInTheDocument();
  expect(screen.getByText(/通常很快归零/)).toBeInTheDocument();
  expect(screen.getByText("1 / 2")).toBeInTheDocument();
  expect(screen.queryByText("服务状态")).not.toBeInTheDocument();
  expect(screen.queryByText("进程阶段")).not.toBeInTheDocument();
  expect(screen.queryByText("运行中")).not.toBeInTheDocument();
  expect(screen.queryByText(/运行态、调用量与调度快照/)).not.toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "调整策略" })).not.toBeInTheDocument();
  expect(rendered.container.querySelector(".rounded-\\[14px\\]")).toBeNull();
  expect(getNotifications()).toHaveLength(0);

  fireEvent.click(screen.getByRole("button", { name: "刷新" }));
  await waitFor(() => {
    expect(getNotifications().map((item) => item.message)).toEqual(["系统状态已刷新"]);
  });
});

test("shows the explicit affinity policy state instead of two misleading zeroes", async () => {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    if (String(input).includes("/api/admin/affinity")) {
      return jsonResponse({
        config_revision: 7,
        affinity_enabled: false,
        active_session_count: 0,
        creating_session_count: 0,
      });
    }
    return jsonResponse({
      status: "ok",
      config_revision: 7,
      scheduler_epoch: 2,
      shutdown_phase: "running",
      active_requests: 1,
      background_tasks: 2,
    });
  });

  render(<SystemOverview />, { wrapper: Wrapper });

  expect(await screen.findByText("已关闭")).toBeInTheDocument();
  expect(screen.getByText("显式会话粘性未启用")).toBeInTheDocument();
  expect(screen.queryByText("当前活动")).not.toBeInTheDocument();
  expect(screen.queryByText("正在建立")).not.toBeInTheDocument();
});

test("rejects an incompatible health payload", async () => {
  vi.spyOn(globalThis, "fetch").mockImplementation(async (input) => {
    const url = String(input);
    if (url.includes("/api/admin/affinity")) {
      return jsonResponse({
        config_revision: 7,
        affinity_enabled: false,
        active_session_count: 0,
        creating_session_count: 0,
      });
    }
    return jsonResponse({ status: "ok" });
  });

  render(<SystemOverview />, { wrapper: Wrapper });

  expect(await screen.findByText("连接失败")).toBeInTheDocument();
});

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
