import { requestJson } from "@/shared/api/http-client";

import { parseBalancingRuntime } from "./balancing-contracts";

export function getBalancingRuntime(signal?: AbortSignal) {
  return requestJson<unknown>("/api/admin/balancing", { signal }).then(parseBalancingRuntime);
}
