import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useEffectEvent } from "react";

import { oauthQueryKeys } from "./oauth-query-keys";

const OAUTH_EVENTS_URL = "/api/admin/oauth/quota-events";
const OAUTH_QUOTA_CHANGED_EVENT = "oauth_quota_changed";
const OAUTH_REFRESH_DIAGNOSTIC_CHANGED_EVENT = "oauth_refresh_diagnostic_changed";

export function useOAuthQuotaChangeEvent() {
  const queryClient = useQueryClient();
  const handleChange = useEffectEvent(() => {
    void queryClient.invalidateQueries({
      queryKey: oauthQueryKeys.quotas,
      refetchType: "active",
    });
  });
  const handleRefreshDiagnosticChange = useEffectEvent(() => {
    void queryClient.invalidateQueries({
      queryKey: oauthQueryKeys.accounts,
      refetchType: "active",
    });
  });

  useEffect(() => {
    if (typeof EventSource === "undefined") {
      return;
    }

    const source = new EventSource(OAUTH_EVENTS_URL);
    source.addEventListener(OAUTH_QUOTA_CHANGED_EVENT, handleChange);
    source.addEventListener(
      OAUTH_REFRESH_DIAGNOSTIC_CHANGED_EVENT,
      handleRefreshDiagnosticChange,
    );
    return () => {
      source.removeEventListener(OAUTH_QUOTA_CHANGED_EVENT, handleChange);
      source.removeEventListener(
        OAUTH_REFRESH_DIAGNOSTIC_CHANGED_EVENT,
        handleRefreshDiagnosticChange,
      );
      source.close();
    };
  }, []);
}
