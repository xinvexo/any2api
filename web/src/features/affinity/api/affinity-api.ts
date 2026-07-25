import { requestJson } from "@/shared/api/http-client";

import { parseAffinityRuntime } from "./affinity-contracts";

export function getAffinity(signal?: AbortSignal) {
  return requestJson<unknown>("/api/admin/affinity?limit=0", { signal }).then(
    parseAffinityRuntime,
  );
}
