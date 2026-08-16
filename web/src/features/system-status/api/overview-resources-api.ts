import { requestJson } from "@/shared/api/http-client";

import {
  parseOverviewResources,
  type OverviewResources,
} from "./overview-resources-contracts";

export function getOverviewResources(signal?: AbortSignal): Promise<OverviewResources> {
  return requestJson<unknown>("/api/admin/overview/resources", { signal }).then(
    parseOverviewResources,
  );
}
