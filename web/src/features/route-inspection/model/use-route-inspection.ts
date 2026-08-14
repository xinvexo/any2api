import { useQuery } from "@tanstack/react-query";

import { getRouteInspection } from "../api/route-inspection-api";
import { routeInspectionQueryKeys } from "./route-inspection-query-keys";

export function useRouteInspection() {
  return useQuery({
    queryKey: routeInspectionQueryKeys.current(),
    queryFn: ({ signal }) => getRouteInspection(signal),
  });
}
