import {
  parseOverviewUsage,
  type OverviewUsageRange,
} from "./overview-usage-contracts";
import { requestJson } from "@/shared/api/http-client";

export function getOverviewUsage(range: OverviewUsageRange, signal?: AbortSignal) {
  return requestJson<unknown>(`/api/admin/overview/usage?range=${range}`, { signal }).then(
    (value) => parseOverviewUsage(value, range),
  );
}
