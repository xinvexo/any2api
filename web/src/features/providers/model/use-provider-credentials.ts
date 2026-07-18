import { useQuery } from "@tanstack/react-query";

import { listProviderCredentials } from "../api/provider-credential-api";
import { providerQueryKeys } from "./provider-query-keys";

export function useProviderCredentials(endpointId: string) {
  return useQuery({
    queryKey: providerQueryKeys.credentials(endpointId),
    queryFn: ({ signal }) => listProviderCredentials(endpointId, signal),
  });
}
