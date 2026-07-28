import { useQuery } from "@tanstack/react-query";

import { getOverviewUsage } from "../api/overview-usage-api";
import type { OverviewUsageRange } from "../api/overview-usage-contracts";
import { overviewUsageQueryKeys } from "./overview-usage-query-keys";

export function useOverviewUsage(range: OverviewUsageRange) {
  return useQuery({
    queryKey: overviewUsageQueryKeys.range(range),
    queryFn: ({ signal }) => getOverviewUsage(range, signal),
    refetchInterval: 60_000,
  });
}
