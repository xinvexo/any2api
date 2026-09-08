import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { getRuntimeLogs } from "../api/runtime-log-api";
import { RuntimeLogManagement } from "./RuntimeLogManagement";
import type { RuntimeLogEntry } from "@/shared/api/generated/RuntimeLogEntry";

vi.mock("../api/runtime-log-api", () => ({ getRuntimeLogs: vi.fn() }));
const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
afterEach(() => { client.clear(); vi.clearAllMocks(); });

const entry: RuntimeLogEntry = {
  id: "event-1", timestamp: "2026-09-08T02:30:00Z", level: "warn", module: "oauth",
  target: "any2api_runtime::oauth::refresh::state", message: "OAuth account token refresh failed",
  summary: "账号凭据刷新失败",
  fields: { oauth_account_id: "account-a", oauth_account_name: "主账号", refresh_stage: "TokenRequest", refresh_reason: "Rejected", reauthorization_required: "true", upstream_status: "401" },
};

test("opens actionable runtime diagnostics", async () => {
  vi.mocked(getRuntimeLogs).mockResolvedValue({ items: [entry], next_cursor: null });
  render(<QueryClientProvider client={client}><MemoryRouter><RuntimeLogManagement /></MemoryRouter></QueryClientProvider>);
  fireEvent.click(await screen.findByRole("button", { name: "查看事件：账号凭据刷新失败" }));
  const drawer = await screen.findByRole("dialog", { name: "运行日志详情" });
  expect(within(drawer).getByText("主账号")).toBeInTheDocument();
  expect(within(drawer).getByText("401")).toBeInTheDocument();
  expect(within(drawer).getByRole("link", { name: "查看 OAuth 账号" })).toHaveAttribute("href", "/oauth");
  fireEvent.click(within(drawer).getByRole("button", { name: "关闭" }));
});

test("loads older pages when the current scan has no application events", async () => {
  vi.mocked(getRuntimeLogs).mockResolvedValueOnce({ items: [], next_cursor: "older" }).mockResolvedValue({ items: [entry], next_cursor: null });
  render(<QueryClientProvider client={client}><MemoryRouter><RuntimeLogManagement /></MemoryRouter></QueryClientProvider>);
  fireEvent.click(await screen.findByRole("button", { name: "加载更早日志" }));
  expect(await screen.findByRole("button", { name: "查看事件：账号凭据刷新失败" })).toBeInTheDocument();
  expect(getRuntimeLogs).toHaveBeenLastCalledWith("older", expect.any(AbortSignal));
});

test("loads older events when the current page does not fill the viewport", async () => {
  vi.mocked(getRuntimeLogs).mockResolvedValueOnce({ items: [entry], next_cursor: "older" }).mockResolvedValue({ items: [{ ...entry, id: "event-older", summary: "更早的事件" }], next_cursor: null });
  render(<QueryClientProvider client={client}><MemoryRouter><RuntimeLogManagement /></MemoryRouter></QueryClientProvider>);
  fireEvent.click(await screen.findByRole("button", { name: "加载更早日志" }));
  expect(await screen.findByRole("button", { name: "查看事件：更早的事件" })).toBeInTheDocument();
});
