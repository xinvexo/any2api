import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type {
  ActiveRequestLog,
  RequestLog,
} from "../api/request-log-contracts";
import { RequestLogVirtualTable } from "./RequestLogVirtualTable";

afterEach(() => vi.useRealTimers());

test("renders only visible request rows and selects a row without expanding it", async () => {
  const items = Array.from({ length: 200 }, (_, index) => requestLog(index + 1));
  const onSelect = vi.fn();
  render(
    <div className="h-[320px]">
      <RequestLogVirtualTable
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

  const viewport = screen.getByRole("rowgroup", { name: "请求日志表格数据" });
  const firstRow = within(viewport).getByRole("row", { name: "查看请求 model-1" });
  expect(firstRow).toHaveAttribute("aria-rowindex", "2");
  fireEvent.click(firstRow);
  expect(onSelect).not.toHaveBeenCalled();
  fireEvent.doubleClick(firstRow);
  expect(onSelect).toHaveBeenCalledWith("request-1");
  expect(within(viewport).getAllByRole("row").length).toBeLessThan(40);
  expect(within(viewport).queryByText("model-200")).not.toBeInTheDocument();

  viewport.scrollTop = 7_800;
  fireEvent.scroll(viewport);

  await waitFor(() => expect(within(viewport).getByText("model-200")).toBeInTheDocument());
  expect(within(viewport).getByRole("row", { name: "查看请求 model-200" }))
    .toHaveAttribute("aria-rowindex", "201");
  expect(within(viewport).queryByText("model-1")).not.toBeInTheDocument();
});

test("renders request metrics and advances the active request duration", async () => {
  const active = activeRequestLog();
  const completed = requestLog(1);
  vi.useFakeTimers();
  vi.setSystemTime(active.startedAtMs + 1_000);
  render(
    <div className="h-[320px]">
      <RequestLogVirtualTable
        items={[active, completed]}
        selectedId={null}
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
  const column = (name: string) => headers.findIndex((header) => header.textContent === name);
  const activeCells = within(activeRow as HTMLElement).getAllByRole("cell");
  expect(activeCells).toHaveLength(headers.length);
  expect(activeCells[column("客户端 IP")]).toHaveAttribute("title", active.clientIp);
  expect(within(activeCells[column("模型")]!).queryByText("流")).not.toBeInTheDocument();
  expect(
    within(activeCells[column("模型")]!).getByLabelText("Fast 模式"),
  ).toHaveTextContent("Fast");
  expect(activeCells[column("结果")]).toHaveTextContent("请求中");
  expect(activeCells[column("总耗时")]).toHaveTextContent("1.00 s");
  expect(activeCells[column("首字")]).toHaveTextContent("—");
  expect(activeCells[column("估算费用")]).toHaveTextContent("—");
  const completedCells = within(completedRow).getAllByRole("cell");
  expect(completedCells).toHaveLength(headers.length);
  expect(completedCells[column("客户端 IP")]).toHaveAttribute("title", completed.clientIp);
  const model = within(completedCells[column("模型")]!);
  expect(model.getByLabelText("Fast 模式")).toHaveTextContent("Fast");
  expect(model.getByLabelText("请求模式：流式")).toHaveTextContent("流");
  expect(model.getByLabelText("思考深度：high")).toHaveTextContent("high");
  expect(completedCells[column("总耗时")]).toHaveTextContent("10 ms");
  expect(completedCells[column("首字")]).toHaveTextContent("2 ms");
  expect(completedCells[column("缓存命中")]).toHaveTextContent("0");
  expect(completedCells[column("输出")]).toHaveTextContent("1");
  expect(completedCells[column("估算费用")]).toHaveTextContent("$0.4");
  expect(completedCells[column("估算费用")]).toHaveAttribute("title", "10 Credits · Fast");
  await act(async () => vi.advanceTimersByTimeAsync(1_000));
  expect(activeCells[column("总耗时")]).toHaveTextContent("2.00 s");
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
    thinkingLevel: "high",
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
    quotaCost: {
      unit: "codex_credits",
      amountNanos: "10000000000",
      rateCard: "codex-rate-2026-08",
      serviceTier: "fast",
      creditsPerUsd: 25,
    },
    isStream: true,
    requestedSpeedTier: "fast",
    effectiveSpeedTier: "fast",
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
    isStream: null,
    requestedSpeedTier: "fast",
    effectiveSpeedTier: null,
  };
}
