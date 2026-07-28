import { describe, expect, test } from "vitest";

import type { ProviderEndpoint } from "@/features/providers";
import type { ProxyProfile, ProxyTestResult } from "../api/proxy-contracts";
import { isCurrentTestResult } from "./proxy-test-result";

describe("isCurrentTestResult", () => {
  test("accepts a result from the current configuration and resource versions", () => {
    expect(isCurrentTestResult(result, proxy, 7, [endpoint], endpoint.id)).toBe(true);
  });

  test.each([
    ["configuration revision", { ...result, configRevision: 8 }, proxy, endpoint],
    ["proxy config version", { ...result, proxyConfigVersion: 3 }, proxy, endpoint],
    [
      "Endpoint config version",
      result,
      proxy,
      { ...endpoint, configVersion: endpoint.configVersion + 1 },
    ],
  ])("rejects a stale %s", (_name, candidate, currentProxy, currentEndpoint) => {
    expect(
      isCurrentTestResult(
        candidate as ProxyTestResult,
        currentProxy as ProxyProfile,
        7,
        [currentEndpoint as ProviderEndpoint],
        endpoint.id,
      ),
    ).toBe(false);
  });
});

const proxy: ProxyProfile = {
  id: "a81bf8f8-8fb4-45f0-926d-1cfda84884f5",
  name: "HTTP",
  kind: "http",
  host: "proxy.example.com",
  port: 8080,
  username: null,
  passwordConfigured: false,
  authenticationVersion: 0,
  enabled: true,
  builtIn: false,
  configVersion: 2,
};

const endpoint: ProviderEndpoint = {
  id: "7dd71e36-cc35-4727-903c-9555ab17290a",
  name: "Codex",
  providerKind: "codex",
  baseUrl: "https://api.openai.com/v1",
  protocolDialect: "openai_responses",
  upstreamProtocolDialect: null,
  enabled: true,
  configVersion: 4,
};

const result: ProxyTestResult = {
  configRevision: 7,
  proxyConfigVersion: proxy.configVersion,
  providerEndpointConfigVersion: endpoint.configVersion,
  proxyId: proxy.id,
  providerEndpointId: endpoint.id,
  reachable: true,
  statusCode: 204,
  latencyMs: 18,
  errorStage: null,
  failureScope: null,
};
