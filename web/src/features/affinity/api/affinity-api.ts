import { requestJson } from "@/shared/api/http-client";

import {
  parseAffinityClearResult,
  parseAffinityRuntime,
} from "./affinity-contracts";

export function getAffinity(limit = 100, signal?: AbortSignal) {
  return requestJson<unknown>(`/api/admin/affinity?limit=${limit}`, { signal }).then(
    parseAffinityRuntime,
  );
}

export function clearAllAffinity() {
  return requestJson<unknown>("/api/admin/affinity", { method: "DELETE" }).then(
    parseAffinityClearResult,
  );
}

export function clearCredentialAffinity(credentialId: string) {
  return requestJson<unknown>(
    `/api/admin/affinity/credentials/${encodeURIComponent(credentialId)}`,
    { method: "DELETE" },
  ).then(parseAffinityClearResult);
}
