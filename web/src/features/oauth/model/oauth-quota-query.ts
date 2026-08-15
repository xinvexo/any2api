import { queryOptions, type QueryClient } from "@tanstack/react-query";

import {
  getOAuthAccountQuota,
  refreshOAuthAccountQuotaRequest,
} from "../api/oauth-api";
import type {
  OAuthAccountConfiguration,
  OAuthRefreshFailure,
} from "../api/oauth-contracts";
import { parseOAuthRefreshFailure } from "../api/oauth-refresh-contracts";
import { oauthQueryKeys } from "./oauth-query-keys";
import { ApiError } from "@/shared/api/http-client";

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
  try {
    const result = await refreshOAuthAccountQuotaRequest(accountId);
    queryClient.setQueryData(oauthQueryKeys.quota(accountId), result.snapshot);
    await queryClient.invalidateQueries({
      queryKey: oauthQueryKeys.accounts,
      refetchType: "active",
    });
    return result;
  } catch (error) {
    applyRefreshDiagnostic(queryClient, accountId, error);
    throw error;
  }
}

function applyRefreshDiagnostic(
  queryClient: QueryClient,
  accountId: string,
  error: unknown,
) {
  if (!(error instanceof ApiError) || error.diagnostic === null) {
    return;
  }
  const diagnostic = error.diagnostic;
  let failure: OAuthRefreshFailure | null;
  try {
    failure = parseOAuthRefreshFailure({
      token_version: diagnostic.tokenVersion,
      trigger: diagnostic.trigger,
      stage: diagnostic.stage,
      reason: diagnostic.reason,
      upstream_status: diagnostic.upstreamStatus,
      failure_scope: diagnostic.failureScope,
      occurred_at: diagnostic.occurredAt,
      reauthorization_required: diagnostic.reauthorizationRequired,
    });
  } catch {
    return;
  }
  if (failure === null) {
    return;
  }

  queryClient.setQueryData<OAuthAccountConfiguration>(
    oauthQueryKeys.accounts,
    (configuration) => {
      if (configuration === undefined) {
        return configuration;
      }
      let changed = false;
      const items = configuration.items.map((account) => {
        if (
          account.id !== accountId
          || account.tokenVersion !== failure.tokenVersion
        ) {
          return account;
        }
        changed = true;
        return { ...account, tokenRefreshFailure: failure };
      });
      return changed ? { ...configuration, items } : configuration;
    },
  );
}
