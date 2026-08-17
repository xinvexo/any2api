import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { expect, test } from "vitest";

import type { RequestLogList } from "../api/request-log-contracts";
import { requestLogQueryKeys } from "../model/request-log-query-keys";
import { RequestLogManagement } from "./RequestLogManagement";
import { AdminRealtimeProvider } from "@/shared/realtime";

test("shows compact metrics and opens request details in a drawer", async () => {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity, gcTime: Infinity },
    },
  });
  const logs = requestLogs();
  client.setQueryData(requestLogQueryKeys.list(), {
    pages: [logs],
    pageParams: [null],
  });
  client.setQueryData(requestLogQueryKeys.detail(logs.items[0].requestId), {
    request: logs.items[0],
    attempts: [],
    telemetry: logs.telemetry,
  });

  render(
    <MemoryRouter>
      <QueryClientProvider client={client}>
        <AdminRealtimeProvider authenticated={false}>
          <RequestLogManagement />
        </AdminRealtimeProvider>
      </QueryClientProvider>
    </MemoryRouter>,
  );

  expect(screen.getByRole("columnheader", { name: "客户端 IP" })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "输入 Token" })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "输出 Token" })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "缓存命中 Token" })).toBeInTheDocument();
  expect(screen.getByRole("cell", { name: "203.0.113.8" })).toBeInTheDocument();
  expect(within(screen.getByRole("list", { name: "请求日志列表" })).getByText("claude-test")).toBeInTheDocument();
  expect(screen.getAllByText("请求中").length).toBeGreaterThan(0);
  expect(screen.queryByLabelText(/展开 codex-live/)).not.toBeInTheDocument();

  fireEvent.click(screen.getByRole("row", { name: "查看请求 claude-test" }));
  const drawer = await screen.findByRole("dialog", { name: "请求详情" });
  expect(within(drawer).getByText("Gateway API Key")).toBeInTheDocument();
  expect(within(drawer).getByText("Provider Endpoint")).toBeInTheDocument();
  expect(within(drawer).getByText("日志遥测")).toBeInTheDocument();
});

function requestLogs(): RequestLogList {
  return {
    activeItems: [
      {
        state: "processing",
        requestId: "55555555-5555-4555-8555-555555555555",
        startedAtMs: 1_700_000_000_100,
        clientIp: "203.0.113.9",
        configRevision: 9,
        gatewayApiKeyId: "22222222-2222-4222-8222-222222222222",
        ingressProtocol: "openai_responses",
        operation: "responses",
        publicModel: "codex-live",
        thinkingLevel: "high",
        providerEndpointId: null,
        providerEndpointName: null,
        credentialId: null,
        credentialLabel: null,
        oauthAccountId: null,
        oauthAccountLabel: null,
        proxyProfileId: null,
        proxyProfileLabel: null,
        attemptCount: 0,
        isStream: true,
      },
    ],
    activeTotal: 1,
    items: [
      {
        requestId: "11111111-1111-4111-8111-111111111111",
        startedAtMs: 1_700_000_000_000,
        clientIp: "203.0.113.8",
        configRevision: 9,
        gatewayApiKeyId: "22222222-2222-4222-8222-222222222222",
        ingressProtocol: "anthropic_messages",
        operation: "messages",
        publicModel: "claude-test",
        thinkingLevel: null,
        providerEndpointId: "endpoint-1",
        providerEndpointName: "Claude",
        credentialId: "credential-1",
        credentialLabel: "primary",
        oauthAccountId: null,
        oauthAccountLabel: null,
        proxyProfileId: null,
        proxyProfileLabel: null,
        statusCode: 200,
        outcome: "success",
        errorMessage: null,
        attemptCount: 1,
        latencyMs: 2_000,
        firstTokenMs: 500,
        inputTokens: 125,
        outputTokens: 25,
        cacheReadTokens: 100,
        cacheCreationTokens: null,
        isStream: true,
      },
    ],
    nextCursor: null,
    hasMore: false,
    telemetry: {
      queuedRecords: 0,
      inFlightRecords: 0,
      droppedRecords: 0,
      persistedRecords: 1,
    },
    filterOptions: {
      publicModels: ["claude-test"],
      gatewayApiKeys: [],
    },
  };
}
