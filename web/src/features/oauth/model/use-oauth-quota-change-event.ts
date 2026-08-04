import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useEffectEvent } from "react";

import { oauthQueryKeys } from "./oauth-query-keys";

const OAUTH_QUOTA_EVENTS_URL = "/api/admin/oauth/quota-events";
const OAUTH_QUOTA_CHANGED_EVENT = "oauth_quota_changed";

export function useOAuthQuotaChangeEvent() {
  const queryClient = useQueryClient();
  const handleChange = useEffectEvent(() => {
    void queryClient.invalidateQueries({
      queryKey: oauthQueryKeys.quotas,
      refetchType: "active",
    });
  });

  useEffect(() => {
    if (typeof EventSource === "undefined") {
      return;
    }

    const source = new EventSource(OAUTH_QUOTA_EVENTS_URL);
    source.addEventListener(OAUTH_QUOTA_CHANGED_EVENT, handleChange);
    return () => {
      source.removeEventListener(OAUTH_QUOTA_CHANGED_EVENT, handleChange);
      source.close();
    };
  }, []);
}
