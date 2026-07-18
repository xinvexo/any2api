import { useQuery } from "@tanstack/react-query";

import { listModelRoutes } from "../api/model-route-api";
import { modelRouteQueryKeys } from "./model-route-query-keys";

export function useModelRoutes() {
  return useQuery({
    queryKey: modelRouteQueryKeys.list(),
    queryFn: ({ signal }) => listModelRoutes(signal),
  });
}
