import { useQuery } from "@tanstack/react-query";

import { getRouteInspection } from "../api/route-inspection-api";

const routeInspectionQueryKey = ["route-inspection", "current"] as const;

export function useRouteInspection() {
  return useQuery({
    queryKey: routeInspectionQueryKey,
    queryFn: ({ signal }) => getRouteInspection(signal),
  });
}
