import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import type {
  RequestAttempt,
  RequestLogDetail,
  RequestLogOutcome,
} from "../api/request-log-contracts";
import { requestLogQueryKeys } from "../model/request-log-query-keys";
import { RequestLogExpandedPanel } from "./RequestLogExpandedPanel";

const requestId = "11111111-1111-4111-8111-111111111111";

test("shows the full attempt flow when a retried request finally succeeds", () => {
  const value = detail("success", [
    attempt(1, "failed", "first-account", "server is overloaded"),
    attempt(2, "success", "second-account", null),
  ]);

  renderPanel(value);

  expect(screen.getByText("Attempt 时间线")).toBeInTheDocument();
  expect(screen.getByText("server is overloaded")).toBeInTheDocument();
  expect(screen.getByText("first-account")).toBeInTheDocument();
  expect(screen.getByText("second-account")).toBeInTheDocument();
  expect(
    screen.getByText("负载均衡 · 失败范围：凭据 · 决策：重新选路"),
  ).toBeInTheDocument();
});

test("omits the attempt timeline for a direct single-attempt success", () => {
  const value = detail("success", [
    attempt(1, "success", "only-account", null),
  ]);

  renderPanel(value);

  expect(screen.queryByText("客户端 IP")).not.toBeInTheDocument();
  expect(screen.queryByText("203.0.113.8")).not.toBeInTheDocument();
  expect(screen.queryByText("Attempt 时间线")).not.toBeInTheDocument();
  expect(screen.queryByText("only-account")).not.toBeInTheDocument();
});

function renderPanel(value: RequestLogDetail) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, staleTime: Infinity } },
  });
  client.setQueryData(requestLogQueryKeys.detail(requestId), value);

  return render(
    <QueryClientProvider client={client}>
      <RequestLogExpandedPanel
        requestId={requestId}
        outcome={value.request.outcome}
        attemptCount={value.request.attemptCount}
      />
    </QueryClientProvider>,
  );
}

function detail(
  outcome: RequestLogOutcome,
  attempts: RequestAttempt[],
): RequestLogDetail {
  return {
    request: {
      requestId,
      startedAtMs: 1_700_000_000_000,
      clientIp: "203.0.113.8",
      configRevision: 9,
      gatewayApiKeyId: "22222222-2222-4222-8222-222222222222",
      ingressProtocol: "openai_responses",
      operation: "responses",
      publicModel: "gpt-test",
      thinkingLevel: "high",
      providerEndpointId: null,
      providerEndpointName: null,
      credentialId: attempts.at(-1)?.credentialId ?? null,
      credentialLabel: attempts.at(-1)?.credentialLabel ?? null,
      oauthAccountId: null,
      oauthAccountLabel: null,
      proxyProfileId: "00000000-0000-0000-0000-000000000000",
      proxyProfileLabel: "DIRECT",
      statusCode: 200,
      outcome,
      errorMessage: null,
      attemptCount: attempts.length,
      latencyMs: 40,
      firstTokenMs: 20,
      inputTokens: 10,
      outputTokens: 5,
      cacheReadTokens: 0,
      isStream: true,
    },
    attempts,
    telemetry: {
      queuedRecords: 0,
      inFlightRecords: 0,
      droppedRecords: 0,
      persistedRecords: 1,
    },
  };
}

function attempt(
  attemptNo: number,
  outcome: RequestLogOutcome,
  credentialLabel: string,
  errorMessage: string | null,
): RequestAttempt {
  return {
    attemptNo,
    routeTargetId: `target-${attemptNo}`,
    credentialId: `credential-${attemptNo}`,
    credentialLabel,
    oauthAccountId: null,
    oauthAccountLabel: null,
    proxyProfileId: "00000000-0000-0000-0000-000000000000",
    proxyProfileLabel: "DIRECT",
    routingMode: "balanced",
    failureScope: outcome === "failed" ? "credential" : null,
    retryDecision: outcome === "failed" ? "reselect" : null,
    startedAtMs: 1_700_000_000_000 + attemptNo,
    durationMs: 10,
    errorMessage,
    statusCode: 200,
    outcome,
    transport: null,
    streamTiming: null,
  };
}
