import { describe, expect, test } from "vitest";

import type { ProxyProfile, ProxyTestResult } from "../api/proxy-contracts";
import { formatProxyTestDiagnostic, isCurrentTestResult } from "./proxy-test-result";

describe("isCurrentTestResult", () => {
  test("accepts a result from the current proxy configuration generation", () => {
    expect(isCurrentTestResult(result, proxy, 7)).toBe(true);
  });

  test.each([
    ["configuration revision", { ...result, configRevision: 8 }],
    ["proxy config version", { ...result, proxyConfigVersion: 3 }],
    ["proxy identity", { ...result, proxyId: "589f7e46-5433-466a-8856-f643bcf8ab39" }],
  ])("rejects a stale %s", (_name, candidate) => {
    expect(isCurrentTestResult(candidate as ProxyTestResult, proxy, 7)).toBe(false);
  });
});

test("describes a fixed-target failure without calling it a Provider Endpoint", () => {
  expect(
    formatProxyTestDiagnostic({
      ...result,
      reachable: false,
      statusCode: null,
      latencyMs: 31,
      errorStage: "tls",
      failureScope: "probe_target",
    }),
  ).toBe("失败 · TLS · 探测站点 · 31 ms");
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

const result: ProxyTestResult = {
  configRevision: 7,
  proxyConfigVersion: proxy.configVersion,
  proxyId: proxy.id,
  reachable: true,
  statusCode: 204,
  latencyMs: 18,
  errorStage: null,
  failureScope: null,
};
