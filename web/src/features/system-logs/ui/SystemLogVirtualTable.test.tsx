import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { SystemLog } from "../api/system-log-contracts";
import { SystemLogVirtualTable } from "./SystemLogVirtualTable";

test("renders only the visible system log rows", async () => {
  const items = Array.from({ length: 200 }, (_, index) => systemLog(index + 1));
  const onSelect = vi.fn();
  render(
    <div className="h-[320px]">
      <SystemLogVirtualTable
        items={items}
        selectedId={null}
        followingLatest
        hasMore={false}
        loadingMore={false}
        onSelect={onSelect}
        onFollowingLatestChange={() => {}}
        onLoadMore={() => {}}
      />
    </div>,
  );

  const viewport = screen.getByRole("rowgroup", { name: "系统日志表格数据" });
  expect(within(viewport).getByText("/system/1")).toBeInTheDocument();
  expect(within(viewport).queryByText("/system/200")).not.toBeInTheDocument();
  expect(within(viewport).getAllByTitle("255.255.255.255")[0])
    .toHaveTextContent("255.255.255.255");
  expect(within(viewport).getAllByRole("row").length).toBeLessThan(40);

  const firstRow = within(viewport).getByText("/system/1").closest("[role='row']");
  expect(firstRow).not.toBeNull();
  fireEvent.click(firstRow!);
  expect(onSelect).not.toHaveBeenCalled();
  fireEvent.doubleClick(firstRow!);
  expect(onSelect).toHaveBeenCalledWith("request-1");

  viewport.scrollTop = 7_800;
  fireEvent.scroll(viewport);

  await waitFor(() => expect(within(viewport).getByText("/system/200")).toBeInTheDocument());
  expect(within(viewport).queryByText("/system/1")).not.toBeInTheDocument();
});

function systemLog(index: number): SystemLog {
  return {
    requestId: `request-${index}`,
    startedAtMs: 1_700_000_000_000 + index,
    configRevision: 1,
    clientIp: "255.255.255.255",
    method: "GET",
    path: `/system/${index}`,
    uri: `/system/${index}`,
    httpVersion: "HTTP/1.1",
    statusCode: 200,
    durationMs: 2,
    responseBytes: 128,
    outcome: "completed",
    exchangeCaptured: true,
  };
}
