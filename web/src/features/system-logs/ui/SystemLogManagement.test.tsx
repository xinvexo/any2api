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
  useSystemLog: () => ({
    data: undefined,
    isPending: true,
    refetch: vi.fn(),
  }),
  useClearSystemLogs: () => ({
    isPending: false,
    mutate: vi.fn(),
  }),
}));

beforeEach(() => {
  window.localStorage.removeItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY);
  window.localStorage.removeItem(SYSTEM_LOG_AUTO_REFRESH_STORAGE_KEY);
  useSystemLogsMock.mockReset();
  useSystemLogsMock.mockImplementation(
    (
      _autoRefresh: boolean,
      showAdminOperations: boolean,
      cursor: string | null,
      page: number,
      pageSize: number,
    ) => {
      const data = showAdminOperations
        ? {
            items: [],
            total: 40,
            page,
            pageSize,
            cursor: cursor ?? "s4.1.first",
            nextCursor: page === 1 ? "s4.1.next" : null,
            telemetry: {
              queuedRecords: 0,
              inFlightRecords: 0,
              droppedRecords: 0,
              persistedRecords: 40,
            },
          }
        : undefined;
      return {
        data,
        isPending: data === undefined,
        isFetching: data === undefined,
        isPlaceholderData: false,
        isError: false,
        refetch: vi.fn(),
      };
    },
  );
});

test("hides admin operations through the server query and resets pagination", async () => {
  render(<SystemLogManagement />);

  const filter = screen.getByRole("switch", { name: "显示管理操作" });
  expect(filter).toBeChecked();

  fireEvent.click(screen.getByRole("button", { name: "下一页" }));
  await waitFor(() => {
    expect(useSystemLogsMock).toHaveBeenLastCalledWith(true, true, "s4.1.next", 2, 20);
  });

  fireEvent.click(filter);
  await waitFor(() => {
    expect(useSystemLogsMock).toHaveBeenLastCalledWith(true, false, null, 1, 20);
  });
  expect(filter).not.toBeChecked();
  expect(window.localStorage.getItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY)).toBe("false");
});

test("restores the persisted admin activity preference", () => {
  window.localStorage.setItem(SYSTEM_LOG_ADMIN_OPERATIONS_STORAGE_KEY, "false");

  render(<SystemLogManagement />);

  expect(screen.getByRole("switch", { name: "显示管理操作" })).not.toBeChecked();
  expect(useSystemLogsMock).toHaveBeenLastCalledWith(true, false, null, 1, 20);
});
