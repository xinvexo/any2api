import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

import { SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY } from "../model/system-log-admin-operations-preference";
import { SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY } from "../model/system-log-auto-refresh-preference";
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
  window.localStorage.removeItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY);
  useSystemLogsMock.mockReset();
  useSystemLogsMock.mockImplementation(
    (
      _autoRefresh: boolean,
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
        pendingCount: 0,
        refreshLatest: vi.fn().mockResolvedValue(undefined),
        applyPending: vi.fn(),
        fetchNextPage: vi.fn(),
      };
    },
  );
});

test("hides admin operations through a fresh server-side feed", async () => {
  render(<SystemLogManagement />);

  const filter = screen.getByRole("switch", { name: "显示管理操作" });
  expect(filter).toBeChecked();

  fireEvent.click(filter);
  await waitFor(() => {
    expect(useSystemLogsMock).toHaveBeenLastCalledWith(true, false, true);
  });
  expect(filter).not.toBeChecked();
  expect(window.localStorage.getItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY)).toBe("false");
});

test("restores the persisted admin activity preference", () => {
  window.localStorage.setItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY, "false");

  render(<SystemLogManagement />);

  expect(screen.getByRole("switch", { name: "显示管理操作" })).not.toBeChecked();
  expect(useSystemLogsMock).toHaveBeenLastCalledWith(true, false, true);
});
