import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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

  const viewport = screen.getByRole("rowgroup", { name: "请求日志表格数据" });
  viewport.scrollTop = 100;
  fireEvent.scroll(viewport);
  expect(screen.queryByText(/条新日志/)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "回到顶部" }));
  await waitFor(() => expect(viewport.scrollTop).toBe(0));

  const row = screen.getByRole("row", { name: "查看请求 claude-test" });
  fireEvent.click(row);
  expect(screen.queryByRole("dialog", { name: "请求详情" })).not.toBeInTheDocument();
  fireEvent.doubleClick(row);
  const drawer = await screen.findByRole("dialog", { name: "请求详情" });
  expect(within(drawer).getByText("Gateway API Key")).toBeInTheDocument();
  expect(within(drawer).queryByText("Provider Endpoint")).not.toBeInTheDocument();
  expect(within(drawer).getByText("上游凭据")).toBeInTheDocument();
  expect(within(drawer).getByText("Claude · primary")).toBeInTheDocument();
  expect(within(drawer).queryByText("日志遥测")).not.toBeInTheDocument();
});

test("does not show an endpoint placeholder for OAuth requests", async () => {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity, gcTime: Infinity },
    },
  });
  const logs = requestLogs();
  const oauthRequest = {
    ...logs.items[0],
    oauthAccountId: "oauth-account-1",
    oauthAccountLabel: "marking.huge_20@icloud.com",
    providerEndpointId: null,
    providerEndpointName: null,
    credentialId: null,
    credentialLabel: null,
  };
  const oauthLogs = { ...logs, items: [oauthRequest] };
  client.setQueryData(requestLogQueryKeys.list(), {
    pages: [oauthLogs],
    pageParams: [null],
  });
  client.setQueryData(requestLogQueryKeys.detail(oauthRequest.requestId), {
    request: oauthRequest,
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

  fireEvent.doubleClick(screen.getByRole("row", { name: "查看请求 claude-test" }));
  const drawer = await screen.findByRole("dialog", { name: "请求详情" });
  expect(within(drawer).queryByText("Provider Endpoint")).not.toBeInTheDocument();
  expect(within(drawer).getByText("上游凭据")).toBeInTheDocument();
  expect(within(drawer).getByText("OAuth · marking.huge_20@icloud.com")).toBeInTheDocument();
  expect(within(drawer).getAllByText("claude-test")).toHaveLength(1);
  expect(within(drawer).getAllByText(oauthRequest.requestId)).toHaveLength(1);
});

test("keeps failed request details focused on the actual failure", async () => {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity, gcTime: Infinity },
    },
  });
  const logs = requestLogs();
  const failedRequest = {
    ...logs.items[0],
    statusCode: 503,
    outcome: "failed" as const,
    errorMessage: "upstream request failed",
    firstTokenMs: null,
    inputTokens: null,
    outputTokens: null,
    cacheReadTokens: null,
    cacheCreationTokens: null,
  };
  client.setQueryData(requestLogQueryKeys.list(), {
    pages: [{ ...logs, items: [failedRequest] }],
    pageParams: [null],
  });
  client.setQueryData(requestLogQueryKeys.detail(failedRequest.requestId), {
    request: failedRequest,
    attempts: [
      {
        attemptNo: 1,
        routeTargetId: "target-1",
        providerEndpointId: "endpoint-claude",
        providerEndpointName: "Claude",
        credentialId: "credential-1",
        credentialLabel: "key3",
        oauthAccountId: null,
        oauthAccountLabel: null,
        proxyProfileId: null,
        proxyProfileLabel: null,
        routingMode: "balanced",
        failureScope: "exact_candidate",
        retryDecision: "terminal",
        startedAtMs: failedRequest.startedAtMs,
        durationMs: failedRequest.latencyMs,
        errorMessage: failedRequest.errorMessage,
        statusCode: 503,
        outcome: "failed",
        transport: {
          wireProfileId: "generic-rustls-hyper-v3",
          wireProfileVersion: 3,
          timeoutPolicyVersion: 1,
          resolverMode: "proxy_remote",
          proxyKind: "socks5",
          connectTimeoutMs: 10_000,
          readTimeoutMs: 300_000,
          poolIdleTimeoutMs: 50_000,
          routingGeneration: 1,
          authenticationVersion: 1,
          trafficClass: "data_plane",
        },
        streamTiming: null,
      },
    ],
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

  fireEvent.doubleClick(screen.getByRole("row", { name: "查看请求 claude-test" }));
  const drawer = await screen.findByRole("dialog", { name: "请求详情" });
  expect(within(drawer).getAllByText("upstream request failed")).toHaveLength(1);
  expect(within(drawer).queryByText("错误信息")).not.toBeInTheDocument();
  expect(within(drawer).getByText("Claude · key3")).toBeInTheDocument();
  expect(within(drawer).queryByText("首 Token 延迟")).not.toBeInTheDocument();
  expect(within(drawer).queryByText("输入 Token")).not.toBeInTheDocument();
  expect(within(drawer).queryByText("输出 Token")).not.toBeInTheDocument();
  expect(within(drawer).queryByText("TPS")).not.toBeInTheDocument();
  expect(within(drawer).queryByText(/Route/)).not.toBeInTheDocument();
  expect(within(drawer).queryByText(/generic-rustls/)).not.toBeInTheDocument();
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
