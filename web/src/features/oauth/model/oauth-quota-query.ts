import { queryOptions, type QueryClient } from "@tanstack/react-query";

import {
  getOAuthAccountQuota,
  refreshOAuthAccountQuotaRequest,
} from "../api/oauth-api";
import { oauthQueryKeys } from "./oauth-query-keys";

const OAUTH_QUOTA_CACHE_GC_TIME_MS = 60 * 60 * 1_000;

export function oauthQuotaQueryOptions(accountId: string) {
  return queryOptions({
    queryKey: oauthQueryKeys.quota(accountId),
    // This read only loads the latest SQLite snapshot, so mounted account rows
    // can restore quota without contacting a Provider.
    queryFn: () => getOAuthAccountQuota(accountId),
    gcTime: OAUTH_QUOTA_CACHE_GC_TIME_MS,
    retry: false,
    staleTime: Infinity,
  });
}

export async function refreshOAuthAccountQuota(
  queryClient: QueryClient,
  accountId: string,
) {
  // Keep the authoritative POST outside the cache GET query. The server emits
  // a quota-change event after persisting, so an SSE cache read may overlap this
  // command without cancelling it or accidentally turning a refetch into POST.
  const snapshot = await refreshOAuthAccountQuotaRequest(accountId);
  queryClient.setQueryData(oauthQueryKeys.quota(accountId), snapshot);
  return snapshot;
}
