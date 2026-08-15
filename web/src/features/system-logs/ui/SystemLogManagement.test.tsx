import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

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
            cursor: cursor ?? "s3.1.first",
            nextCursor: page === 1 ? "s3.1.next" : null,
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
    expect(useSystemLogsMock).toHaveBeenLastCalledWith(true, true, "s3.1.next", 2, 20);
  });

  fireEvent.click(filter);
  await waitFor(() => {
    expect(useSystemLogsMock).toHaveBeenLastCalledWith(true, false, null, 1, 20);
  });
  expect(filter).not.toBeChecked();
});
