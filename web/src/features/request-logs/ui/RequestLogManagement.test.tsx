import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { expect, test } from "vitest";

import type { RequestLogList } from "../api/request-log-contracts";
import { requestLogQueryKeys } from "../model/request-log-query-keys";
import { RequestLogManagement } from "./RequestLogManagement";

test("shows client IP and explicit token metric names in the request list", () => {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity, gcTime: Infinity },
    },
  });
  client.setQueryData(requestLogQueryKeys.list(null, 1, 20), requestLogs());

  render(
    <MemoryRouter>
      <QueryClientProvider client={client}>
        <RequestLogManagement />
      </QueryClientProvider>
    </MemoryRouter>,
  );

  expect(screen.getByRole("columnheader", { name: "客户端 IP" })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "输入 Token" })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "输出 Token" })).toBeInTheDocument();
  expect(screen.getByRole("columnheader", { name: "缓存命中 Token" })).toBeInTheDocument();
  expect(screen.getByRole("cell", { name: "203.0.113.8" })).toBeInTheDocument();
  expect(
    within(screen.getByRole("list", { name: "请求日志列表" })).getByText(
      "IP 203.0.113.8",
    ),
  ).toBeInTheDocument();
  expect(screen.getAllByText("请求中").length).toBeGreaterThan(0);
  expect(screen.queryByLabelText(/展开 codex-live/)).not.toBeInTheDocument();
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
    total: 1,
    page: 1,
    pageSize: 20,
    cursor: "r3.cursor",
    nextCursor: null,
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
