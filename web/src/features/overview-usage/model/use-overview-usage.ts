import { keepPreviousData, useQuery } from "@tanstack/react-query";

import { getOverviewUsage } from "../api/overview-usage-api";
import type { OverviewUsageRange } from "../api/overview-usage-contracts";

const overviewUsageQueryKey = (range: OverviewUsageRange) =>
  ["overview", "usage", range] as const;

export function useOverviewUsage(range: OverviewUsageRange) {
  return useQuery({
    queryKey: overviewUsageQueryKey(range),
    queryFn: ({ signal }) => getOverviewUsage(range, signal),
    placeholderData: keepPreviousData,
    refetchInterval: 60_000,
  });
}
