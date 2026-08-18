import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type {
  ActiveRequestLog,
  RequestLog,
} from "../api/request-log-contracts";
import { RequestLogVirtualTable } from "./RequestLogVirtualTable";

test("renders only visible request rows and selects a row without expanding it", async () => {
  const items = Array.from({ length: 200 }, (_, index) => requestLog(index + 1));
  const onSelect = vi.fn();
  render(
    <div className="h-[320px]">
      <RequestLogVirtualTable
        items={items}
        selectedId={null}
        nowMs={Date.now()}
        followingLatest
        hasMore={false}
        loadingMore={false}
        onSelect={onSelect}
        onFollowingLatestChange={() => {}}
        onLoadMore={() => {}}
      />
    </div>,
  );

  const viewport = screen.getByRole("rowgroup", { name: "请求日志表格数据" });
  const firstRow = within(viewport).getByRole("row", { name: "查看请求 model-1" });
  fireEvent.click(firstRow);
  expect(onSelect).not.toHaveBeenCalled();
  fireEvent.doubleClick(firstRow);
  expect(onSelect).toHaveBeenCalledWith("request-1");
  expect(within(viewport).getAllByRole("row").length).toBeLessThan(40);
  expect(within(viewport).queryByText("model-200")).not.toBeInTheDocument();

  viewport.scrollTop = 7_800;
  fireEvent.scroll(viewport);

  await waitFor(() => expect(within(viewport).getByText("model-200")).toBeInTheDocument());
  expect(within(viewport).queryByText("model-1")).not.toBeInTheDocument();
});

test("renders active and completed request metrics in the expected columns", () => {
  const active = activeRequestLog();
  const completed = requestLog(1);
  render(
    <div className="h-[320px]">
      <RequestLogVirtualTable
        items={[active, completed]}
        selectedId={null}
        nowMs={active.startedAtMs + 1_000}
        followingLatest
        hasMore={false}
        loadingMore={false}
        onSelect={() => {}}
        onFollowingLatestChange={() => {}}
        onLoadMore={() => {}}
      />
    </div>,
  );

  const activeRow = screen.getByText("请求中").closest("[role='row']");
  const completedRow = screen.getByRole("row", { name: "查看请求 model-1" });
  const headers = screen.getAllByRole("columnheader");
  expect(headers[6]).toHaveTextContent("总耗时");
  expect(headers[7]).toHaveTextContent("首字");
  expect(headers[8]).toHaveTextContent("输入");
  expect(headers[9]).toHaveTextContent("缓存命中");
  expect(headers[10]).toHaveTextContent("输出");
  const activeCells = within(activeRow as HTMLElement).getAllByRole("cell");
  expect(activeCells[6]).toHaveTextContent("1.00 s");
  expect(activeCells[7]).toHaveTextContent("-");
  const completedCells = within(completedRow).getAllByRole("cell");
  expect(completedCells[6]).toHaveTextContent("10 ms");
  expect(completedCells[7]).toHaveTextContent("2 ms");
  expect(completedCells[9]).toHaveTextContent("0");
  expect(completedCells[10]).toHaveTextContent("1");
});

function requestLog(index: number): RequestLog {
  return {
    requestId: `request-${index}`,
    startedAtMs: 1_700_000_000_000 - index,
    clientIp: "127.0.0.1",
    configRevision: 1,
    gatewayApiKeyId: null,
    ingressProtocol: "openai_responses",
    operation: "responses",
    publicModel: `model-${index}`,
    thinkingLevel: null,
    providerEndpointId: null,
    providerEndpointName: null,
    credentialId: null,
    credentialLabel: null,
    oauthAccountId: null,
    oauthAccountLabel: null,
    proxyProfileId: null,
    proxyProfileLabel: null,
    statusCode: 200,
    outcome: "success",
    errorMessage: null,
    attemptCount: 1,
    latencyMs: 10,
    firstTokenMs: 2,
    inputTokens: 1,
    outputTokens: 1,
    cacheReadTokens: 0,
    cacheCreationTokens: 0,
    isStream: true,
  };
}

function activeRequestLog(): ActiveRequestLog {
  return {
    state: "processing",
    requestId: "request-active",
    startedAtMs: 1_700_000_000_000,
    clientIp: "127.0.0.1",
    configRevision: 1,
    gatewayApiKeyId: "gateway-key-1",
    ingressProtocol: "openai_responses",
    operation: "responses",
    publicModel: "model-active",
    thinkingLevel: null,
    providerEndpointId: null,
    providerEndpointName: null,
    credentialId: null,
    credentialLabel: null,
    oauthAccountId: null,
    oauthAccountLabel: null,
    proxyProfileId: null,
    proxyProfileLabel: null,
    attemptCount: 1,
    isStream: true,
  };
}
