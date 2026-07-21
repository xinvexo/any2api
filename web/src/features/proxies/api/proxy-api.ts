import { requestJson } from "@/shared/api/http-client";

import {
  type ProxyConfiguration,
  type ProxyAuthenticationInput,
  type ProxyTestResult,
  type ProxyWriteInput,
  parseProxyTestResult,
  parseProxyConfiguration,
} from "./proxy-contracts";

export function listProxies(signal?: AbortSignal) {
  return requestJson<unknown>("/api/admin/proxies", { signal }).then(parseProxyConfiguration);
}

export function createProxy(input: ProxyWriteInput) {
  return writeProxy("/api/admin/proxies", "POST", input);
}

export function updateProxy(id: string, input: ProxyWriteInput) {
  return writeProxy(`/api/admin/proxies/${encodeURIComponent(id)}`, "PATCH", input);
}

export function deleteProxy(id: string, expectedRevision: number) {
  return requestJson<unknown>(
    `/api/admin/proxies/${encodeURIComponent(id)}?expected_revision=${expectedRevision}`,
    { method: "DELETE" },
  ).then(parseProxyConfiguration);
}

export function setGlobalProxy(id: string, expectedRevision: number) {
  return requestJson<unknown>(`/api/admin/proxies/${encodeURIComponent(id)}/set-global`, {
    method: "POST",
    body: { expected_revision: expectedRevision },
  }).then(parseProxyConfiguration);
}

export function setProxyAuthentication(
  id: string,
  expectedRevision: number,
  input: ProxyAuthenticationInput,
) {
  return requestJson<unknown>(`/api/admin/proxies/${encodeURIComponent(id)}/authentication`, {
    method: "PUT",
    body: {
      expected_revision: expectedRevision,
      username: input.username,
      password: input.password,
    },
  }).then(parseProxyConfiguration);
}

export function clearProxyAuthentication(id: string, expectedRevision: number) {
  return requestJson<unknown>(
    `/api/admin/proxies/${encodeURIComponent(id)}/authentication?expected_revision=${expectedRevision}`,
    { method: "DELETE" },
  ).then(parseProxyConfiguration);
}

export function testProxy(id: string, providerEndpointId: string): Promise<ProxyTestResult> {
  return requestJson<unknown>(`/api/admin/proxies/${encodeURIComponent(id)}/test`, {
    method: "POST",
    // The server applies the configurable upstream read timeout (up to 24h).
    timeoutMs: 86_410_000,
    body: { provider_endpoint_id: providerEndpointId },
  }).then(parseProxyTestResult);
}

function writeProxy(path: string, method: string, input: ProxyWriteInput): Promise<ProxyConfiguration> {
  return requestJson<unknown>(path, {
    method,
    body: {
      expected_revision: input.expectedRevision,
      name: input.name,
      kind: input.kind,
      host: input.host,
      port: input.port,
      enabled: input.enabled,
    },
  }).then(parseProxyConfiguration);
}
