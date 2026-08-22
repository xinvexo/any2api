import { useQuery, useQueryClient } from "@tanstack/react-query";

import { parseOverviewResources } from "../api/overview-resources-contracts";
import { getOverviewResources } from "../api/overview-resources-api";
import { useAdminEvent } from "@/shared/realtime";

export const overviewResourcesQueryKey = ["overview-resources", "current"] as const;

export function useOverviewResources() {
  const queryClient = useQueryClient();
  useAdminEvent("overview_snapshot", true, (payload) => {
    const snapshot = record(payload);
    if (!snapshot || !("resources" in snapshot)) {
      return;
    }
    try {
      queryClient.setQueryData(
        overviewResourcesQueryKey,
        parseOverviewResources(snapshot.resources),
      );
    } catch {
      // Ignore malformed realtime data; the HTTP query remains the fallback.
    }
  });

  return useQuery({
    queryKey: overviewResourcesQueryKey,
    queryFn: ({ signal }) => getOverviewResources(signal),
  });
}

function record(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}
