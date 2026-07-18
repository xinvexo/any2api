import { useQuery } from "@tanstack/react-query";

import { listProviderEndpoints } from "../api/provider-api";
import { providerQueryKeys } from "./provider-query-keys";

export function useProviderEndpoints() {
  return useQuery({
    queryKey: providerQueryKeys.list(),
    queryFn: ({ signal }) => listProviderEndpoints(signal),
  });
}
