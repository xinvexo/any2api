import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { ProviderCredential } from "../api/provider-credential-contracts";
import type { ProviderEndpoint } from "../api/provider-contracts";
import {
  providerCredentialTestScope,
  useProviderCredentialTest,
} from "./use-provider-credential-test";
import type { ProxyConfiguration } from "@/features/proxies";

afterEach(() => {
  vi.restoreAllMocks();
});

test("ignores an older request that finishes after a newer request", async () => {
  const first = deferred<Response>();
  const second = deferred<Response>();
  const responses = [first, second];
  vi.spyOn(globalThis, "fetch").mockImplementation(() => {
    const response = responses.shift();
    if (!response) {
      throw new Error("unexpected credential probe");
    }
    return response.promise;
  });
  const { result } = renderHook(() => useProviderCredentialTest("credential-scope"));

  let firstRequest!: Promise<unknown>;
  let secondRequest!: Promise<unknown>;
  act(() => {
    firstRequest = result.current.test(CREDENTIAL_ID);
    secondRequest = result.current.test(CREDENTIAL_ID);
  });

  await act(async () => {
    second.resolve(jsonResponse(testResult(["new-model"])));
    await secondRequest;
  });
  expect(result.current.results[CREDENTIAL_ID]?.models).toEqual(["new-model"]);

  await act(async () => {
    first.resolve(jsonResponse(testResult(["old-model"])));
    await firstRequest;
  });
  expect(result.current.results[CREDENTIAL_ID]?.models).toEqual(["new-model"]);
});

test("keys probe state to the credential's effective resources instead of global revision", () => {
  const current = providerCredentialTestScope(ENDPOINT, CREDENTIAL, PROXIES);
  const unrelatedRevision = providerCredentialTestScope(ENDPOINT, CREDENTIAL, {
    ...PROXIES,
    configRevision: PROXIES.configRevision + 1,
    items: PROXIES.items.map((proxy) =>
      proxy.id === UNUSED_PROXY_ID ? { ...proxy, configVersion: proxy.configVersion + 1 } : proxy,
    ),
  });
  const endpointChanged = providerCredentialTestScope(
    { ...ENDPOINT, configVersion: ENDPOINT.configVersion + 1 },
    CREDENTIAL,
    PROXIES,
  );
  const credentialChanged = providerCredentialTestScope(
    ENDPOINT,
    { ...CREDENTIAL, secretVersion: CREDENTIAL.secretVersion + 1 },
    PROXIES,
  );
  const effectiveProxyChanged = providerCredentialTestScope(ENDPOINT, CREDENTIAL, {
    ...PROXIES,
    items: PROXIES.items.map((proxy) =>
      proxy.id === GLOBAL_PROXY_ID ? { ...proxy, configVersion: proxy.configVersion + 1 } : proxy,
    ),
  });

  expect(unrelatedRevision).toBe(current);
  expect(endpointChanged).not.toBe(current);
  expect(credentialChanged).not.toBe(current);
  expect(effectiveProxyChanged).not.toBe(current);
});

const CREDENTIAL_ID = "75072ca7-d922-428d-a4f8-86401567da32";
const ENDPOINT_ID = "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f";
const DIRECT_PROXY_ID = "00000000-0000-0000-0000-000000000000";
const GLOBAL_PROXY_ID = "f0335fed-e5a9-4081-966b-37efe4a109a8";
const UNUSED_PROXY_ID = "f1b19dd0-a405-46c9-b562-62dc38d7aa40";

const ENDPOINT = {
  id: ENDPOINT_ID,
  name: "Codex Primary",
  providerKind: "codex",
  baseUrl: "https://api.example.com",
  protocolDialect: "openai_responses",
  upstreamProtocolDialect: null,
  enabled: true,
  configVersion: 1,
} satisfies ProviderEndpoint;

const CREDENTIAL = {
  id: CREDENTIAL_ID,
  providerEndpointId: ENDPOINT_ID,
  label: "Primary Key",
  credentialKind: "api_key",
  fingerprint: "v2:0123456789abcdef",
  secretTail: "test",
  proxyProfileId: DIRECT_PROXY_ID,
  requestsPerMinute: 4,
  enabled: true,
  secretVersion: 1,
  credentialGeneration: 1,
  configVersion: 1,
  models: [],
  usage: {
    totalRequests: 0,
    successfulRequests: 0,
    failedRequests: 0,
    windowMinutes: 2,
    windowSlots: [],
  },
} satisfies ProviderCredential;

const PROXIES = {
  configRevision: 2,
  globalProxyId: GLOBAL_PROXY_ID,
  items: [
    proxy(DIRECT_PROXY_ID, "direct", 1),
    proxy(GLOBAL_PROXY_ID, "http", 2),
    proxy(UNUSED_PROXY_ID, "http", 3),
  ],
} satisfies ProxyConfiguration;

function proxy(id: string, kind: "direct" | "http", configVersion: number) {
  return {
    id,
    name: kind === "direct" ? "DIRECT" : `Proxy ${configVersion}`,
    kind,
    host: kind === "direct" ? null : "proxy.example.com",
    port: kind === "direct" ? null : 8080,
    username: null,
    passwordConfigured: false,
    authenticationVersion: 0,
    enabled: true,
    builtIn: kind === "direct",
    configVersion,
  };
}

function testResult(models: string[]) {
  return {
    config_revision: 3,
    provider_endpoint_config_version: 1,
    credential_config_version: 1,
    credential_generation: 1,
    secret_version: 1,
    proxy_config_version: 1,
    credential_id: CREDENTIAL_ID,
    provider_endpoint_id: ENDPOINT_ID,
    proxy_id: DIRECT_PROXY_ID,
    reachable: true,
    accepted: true,
    catalog_valid: true,
    status_code: 200,
    latency_ms: 18,
    auth_error_cleared: true,
    error_stage: null,
    failure_scope: null,
    models,
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
