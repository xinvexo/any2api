import type { OverviewUsageRange } from "../api/overview-usage-contracts";

export const overviewUsageQueryKeys = {
  all: ["overview", "usage"] as const,
  range: (range: OverviewUsageRange) => [...overviewUsageQueryKeys.all, range] as const,
};
