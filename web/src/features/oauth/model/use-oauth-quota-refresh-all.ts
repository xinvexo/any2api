import { useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";

import { runOAuthQuotaBatch } from "./oauth-quota-batch";
import { refreshOAuthAccountQuota } from "./oauth-quota-query";
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
      const outcomes = await runOAuthQuotaBatch(accountIds, (accountId) =>
        refreshOAuthAccountQuota(queryClient, accountId),
      );
      if (outcomes.some((outcome) => outcome.status === "rejected")) {
        await queryClient.invalidateQueries({
          queryKey: oauthQueryKeys.accounts,
          refetchType: "active",
        });
      }
      return {
        total: accountIds.length,
        failed: outcomes.filter((outcome) => outcome.status === "rejected").length,
      };
    } finally {
      setPending(false);
      pendingRef.current = false;
    }
  }

  return { pending, refresh };
}
