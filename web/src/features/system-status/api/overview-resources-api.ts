import { requestJson } from "@/shared/api/http-client";
import type { OverviewResourcesResponse } from "@/shared/api/generated/OverviewResourcesResponse";

import {
  parseOverviewResources,
  type OverviewResources,
} from "./overview-resources-contracts";

export function getOverviewResources(signal?: AbortSignal): Promise<OverviewResources> {
  return requestJson<OverviewResourcesResponse>("/api/admin/overview/resources", { signal }).then(
    parseOverviewResources,
  );
}
