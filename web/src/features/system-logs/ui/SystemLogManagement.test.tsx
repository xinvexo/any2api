import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import { SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY } from "../model/system-log-admin-operations-preference";
import { SystemLogManagement } from "./SystemLogManagement";

const { useSystemLogsMock } = vi.hoisted(() => ({
  useSystemLogsMock: vi.fn(),
}));

vi.mock("../model/use-system-logs", () => ({
  useSystemLogs: useSystemLogsMock,
}));

vi.mock("../model/use-system-log", () => ({
  useSystemLog: () => ({
    data: undefined,
    isPending: true,
    refetch: vi.fn(),
  }),
}));

vi.mock("../model/use-clear-system-logs", () => ({
  useClearSystemLogs: () => ({
    isPending: false,
    mutate: vi.fn(),
  }),
}));

vi.mock("@/shared/realtime", () => ({
  useAdminRealtimeStatus: () => ({ connected: true, stale: false }),
}));

beforeEach(() => {
  window.localStorage.removeItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY);
  useSystemLogsMock.mockReset();
  useSystemLogsMock.mockImplementation(
    (
      showAdminOperations: boolean,
    ) => {
      const data = showAdminOperations
        ? {
            pages: [],
            pageParams: [],
          }
        : undefined;
      return {
        data,
        items: [],
        isPending: data === undefined,
        isFetching: data === undefined,
        isFetchingNextPage: false,
        isError: false,
        hasNextPage: false,
        refreshLatest: vi.fn().mockResolvedValue(undefined),
        applyPending: vi.fn(),
        fetchNextPage: vi.fn(),
      };
    },
  );
});

test("hides admin operations through a fresh server-side feed", async () => {
  render(<SystemLogManagement />);

  expect(screen.queryByRole("switch", { name: "自动刷新" })).not.toBeInTheDocument();
  const filter = screen.getByRole("switch", { name: "显示管理操作" });
  expect(filter).toBeChecked();

  fireEvent.click(filter);
  await waitFor(() => {
    expect(useSystemLogsMock).toHaveBeenLastCalledWith(false, true);
  });
  expect(filter).not.toBeChecked();
  expect(window.localStorage.getItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY)).toBe("false");
});

test("restores the persisted admin activity preference", () => {
  window.localStorage.setItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY, "false");

  render(<SystemLogManagement />);

  expect(screen.getByRole("switch", { name: "显示管理操作" })).not.toBeChecked();
  expect(useSystemLogsMock).toHaveBeenLastCalledWith(false, true);
});

test("uses clear-record wording for the destructive log action", async () => {
  render(<SystemLogManagement />);

  fireEvent.click(screen.getByRole("button", { name: "清空记录" }));

  expect(await screen.findByRole("alertdialog", { name: "清空全部系统日志？" }))
    .toBeInTheDocument();
  expect(screen.getByRole("button", { name: "清空" })).toBeInTheDocument();
});

test("returns to the latest feed without rendering a new-log banner", async () => {
  const applyPending = vi.fn();
  useSystemLogsMock.mockReturnValue({
    data: { pages: [], pageParams: [] },
    items: [systemLog()],
    isPending: false,
    isFetching: false,
    isFetchingNextPage: false,
    isError: false,
    hasNextPage: false,
    refreshLatest: vi.fn().mockResolvedValue(undefined),
    applyPending,
    fetchNextPage: vi.fn(),
  });
  render(<SystemLogManagement />);

  const viewport = screen.getByRole("rowgroup", { name: "系统日志表格数据" });
  viewport.scrollTop = 100;
  fireEvent.scroll(viewport);

  expect(screen.queryByText(/条新日志/)).not.toBeInTheDocument();
  fireEvent.click(await screen.findByRole("button", { name: "回到顶部" }));
  expect(applyPending).toHaveBeenCalledTimes(1);
  await waitFor(() => expect(viewport.scrollTop).toBe(0));
});

function systemLog() {
  return {
    requestId: "request-1",
    startedAtMs: 1_700_000_000_000,
    configRevision: 1,
    clientIp: "127.0.0.1",
    method: "GET",
    path: "/v1/models",
    httpVersion: "HTTP/1.1",
    statusCode: 200,
    durationMs: 2,
    responseBytes: 128,
    outcome: "completed" as const,
  };
}
