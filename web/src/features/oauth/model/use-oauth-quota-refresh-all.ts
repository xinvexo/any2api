import { useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";

import { refreshOAuthQuotaBatchRequest } from "../api/oauth-api";
import { oauthQueryKeys } from "./oauth-query-keys";

export interface OAuthQuotaRefreshResult {
  total: number;
  failed: number;
}

export function useOAuthQuotaRefreshAll() {
  const queryClient = useQueryClient();
  const pendingRef = useRef(false);
  const [pending, setPending] = useState(false);

  async function refresh(
    accountIds: readonly string[],
  ): Promise<OAuthQuotaRefreshResult | null> {
    if (pendingRef.current || accountIds.length === 0) {
      return null;
    }
    pendingRef.current = true;
    setPending(true);
    try {
      const result = await refreshOAuthQuotaBatchRequest(accountIds);
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: oauthQueryKeys.accounts,
          refetchType: "active",
        }),
        queryClient.invalidateQueries({
          queryKey: oauthQueryKeys.quotas,
          refetchType: "active",
        }),
      ]);
      return {
        total: accountIds.length,
        failed: result.failedAccountIds.length,
      };
    } finally {
      setPending(false);
      pendingRef.current = false;
    }
  }

  return { pending, refresh };
}
