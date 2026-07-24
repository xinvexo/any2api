import { queryOptions, type QueryClient } from "@tanstack/react-query";

import { getOAuthAccountQuota } from "../api/oauth-api";
import { oauthQueryKeys } from "./oauth-query-keys";

const OAUTH_QUOTA_CACHE_GC_TIME_MS = 60 * 60 * 1_000;

export function oauthQuotaQueryOptions(accountId: string) {
  return queryOptions({
    queryKey: oauthQueryKeys.quota(accountId),
    // Explicit quota refreshes outlive virtual row observers. requestJson still
    // enforces its own bounded timeout, while row unmount must not abort a batch.
    queryFn: () => getOAuthAccountQuota(accountId),
    gcTime: OAUTH_QUOTA_CACHE_GC_TIME_MS,
    retry: false,
    staleTime: 0,
  });
}

export async function refreshOAuthAccountQuota(
  queryClient: QueryClient,
  accountId: string,
) {
  const options = oauthQuotaQueryOptions(accountId);
  await queryClient.invalidateQueries({
    queryKey: options.queryKey,
    exact: true,
    refetchType: "none",
  });
  return queryClient.fetchQuery(options);
}
