import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import type { PropsWithChildren } from "react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { SystemOverview } from "./SystemOverview";

function Wrapper({ children }: PropsWithChildren) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <MemoryRouter>
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    </MemoryRouter>
  );
}

afterEach(() => vi.restoreAllMocks());

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
  expect(screen.getByText("当前活动")).toBeInTheDocument();
  expect(screen.getByText("3")).toBeInTheDocument();
  expect(screen.getByText("正在建立")).toBeInTheDocument();
  expect(screen.getByText("1")).toBeInTheDocument();
  expect(screen.getByText("1 / 2")).toBeInTheDocument();
  expect(screen.queryByText("服务状态")).not.toBeInTheDocument();
  expect(screen.queryByText("进程阶段")).not.toBeInTheDocument();
  expect(screen.queryByText("运行中")).not.toBeInTheDocument();
  expect(screen.queryByText(/运行态、调用量与调度快照/)).not.toBeInTheDocument();
  expect(screen.queryByRole("link", { name: "调整策略" })).not.toBeInTheDocument();
  expect(rendered.container.querySelector(".rounded-\\[14px\\]")).toBeNull();
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
