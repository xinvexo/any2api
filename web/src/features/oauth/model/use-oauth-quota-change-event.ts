import { useQueryClient, type QueryKey } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import { oauthQueryKeys } from "./oauth-query-keys";
import { useAdminEvent } from "@/shared/realtime";

export function useOAuthQuotaChangeEvent() {
  const queryClient = useQueryClient();
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pending = useRef(new Set<QueryKey>());

  useEffect(() => () => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
    pending.current.clear();
  }, []);

  function schedule(queryKey: QueryKey) {
    pending.current.add(queryKey);
    if (timer.current !== null) return;
    timer.current = setTimeout(() => {
      timer.current = null;
      const keys = [...pending.current];
      pending.current.clear();
      for (const key of keys) {
        void queryClient.invalidateQueries({ queryKey: key, refetchType: "active" });
      }
    }, 100);
  }

  useAdminEvent("oauth_quota_changed", true, () => schedule(oauthQueryKeys.quotas));
  useAdminEvent("oauth_refresh_diagnostic_changed", true, () => schedule(oauthQueryKeys.accounts));
}
