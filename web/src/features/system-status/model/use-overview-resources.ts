import { useQuery } from "@tanstack/react-query";

import { getOverviewResources } from "../api/overview-resources-api";
import { overviewResourcesQueryKeys } from "./overview-resources-query-keys";

export const OVERVIEW_RESOURCES_REFRESH_INTERVAL_MS = 5_000;

export function useOverviewResources() {
  return useQuery({
    queryKey: overviewResourcesQueryKeys.current(),
    queryFn: ({ signal }) => getOverviewResources(signal),
    refetchInterval: OVERVIEW_RESOURCES_REFRESH_INTERVAL_MS,
  });
}
