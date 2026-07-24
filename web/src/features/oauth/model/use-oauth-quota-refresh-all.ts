import { useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";

import { refreshOAuthAccountQuota } from "./oauth-quota-query";

const MAX_CONCURRENT_QUOTA_REFRESHES = 6;

export interface OAuthQuotaRefreshResult {
  total: number;
  failed: number;
}

export function useOAuthQuotaRefreshAll() {
  const queryClient = useQueryClient();
  const pendingRef = useRef(false);
  const [pending, setPending] = useState(false);
  const [result, setResult] = useState<OAuthQuotaRefreshResult | null>(null);

  async function refresh(accountIds: readonly string[]) {
    if (pendingRef.current || accountIds.length === 0) {
      return;
    }
    pendingRef.current = true;
    setPending(true);
    setResult(null);
    let nextIndex = 0;
    let failed = 0;
    try {
      const workers = Array.from(
        { length: Math.min(MAX_CONCURRENT_QUOTA_REFRESHES, accountIds.length) },
        async () => {
          while (nextIndex < accountIds.length) {
            const accountId = accountIds[nextIndex];
            nextIndex += 1;
            try {
              await refreshOAuthAccountQuota(queryClient, accountId);
            } catch {
              failed += 1;
            }
          }
        },
      );
      await Promise.all(workers);
      setResult({ total: accountIds.length, failed });
    } finally {
      setPending(false);
      pendingRef.current = false;
    }
  }

  function clearResult() {
    setResult(null);
  }

  return { pending, result, refresh, clearResult };
}
