import {
  parseOverviewUsage,
  type OverviewUsageRange,
} from "./overview-usage-contracts";
import { requestJson } from "@/shared/api/http-client";
import type { OverviewUsageResponse } from "@/shared/api/generated/OverviewUsageResponse";

export function getOverviewUsage(range: OverviewUsageRange, signal?: AbortSignal) {
  return requestJson<OverviewUsageResponse>(`/api/admin/overview/usage?range=${range}`, { signal }).then(
    (value) => parseOverviewUsage(value, range),
  );
}
