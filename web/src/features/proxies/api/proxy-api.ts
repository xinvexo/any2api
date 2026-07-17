import { requestJson } from "@/shared/api/http-client";

import {
  type ProxyConfiguration,
  type ProxyWriteInput,
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
