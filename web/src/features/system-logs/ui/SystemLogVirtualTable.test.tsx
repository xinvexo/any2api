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
  expect(screen.getByRole("columnheader", { name: "客户端 IP" })).toBeInTheDocument();
  expect(within(viewport).getByText("/system/1")).toBeInTheDocument();
  expect(within(viewport).queryByText("/system/200")).not.toBeInTheDocument();
  const clientIp = within(viewport).getAllByTitle("203.0.113.8")[0];
  expect(clientIp).toHaveTextContent("203.0.113.8");
  expect(within(viewport).getAllByRole("row").length).toBeLessThan(40);

  const firstRow = within(viewport).getByText("/system/1").closest("[role='row']");
  expect(firstRow).not.toBeNull();
  expect(firstRow).toHaveAttribute("aria-rowindex", "2");
  fireEvent.click(within(firstRow as HTMLElement).getByRole("button", { name: "查看 HTTP 请求详情 /system/1" }));
  expect(onSelect).toHaveBeenCalledWith("request-1");
  fireEvent.doubleClick(firstRow!);
  expect(onSelect).toHaveBeenCalledWith("request-1");

  viewport.scrollTop = 9_800;
  fireEvent.scroll(viewport);

  await waitFor(() => expect(within(viewport).getByText("/system/200")).toBeInTheDocument());
  expect(within(viewport).getByText("/system/200").closest("[role='row']"))
    .toHaveAttribute("aria-rowindex", "201");
  expect(within(viewport).queryByText("/system/1")).not.toBeInTheDocument();
});

function systemLog(index: number): SystemLog {
  return {
    requestId: `request-${index}`,
    startedAtMs: 1_700_000_000_000 + index,
    configRevision: 1,
    clientIp: "203.0.113.8",
    method: "GET",
    path: `/system/${index}`,
    httpVersion: "HTTP/1.1",
    statusCode: 200,
    durationMs: 2,
    responseBytes: 128,
    outcome: "completed",
  };
}
