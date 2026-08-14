import { requestJson } from "@/shared/api/http-client";

import { parseRouteInspection } from "./route-inspection-contracts";

export function getRouteInspection(signal?: AbortSignal) {
  return requestJson<unknown>("/api/admin/route-inspection", { signal }).then(
    parseRouteInspection,
  );
}
