import { useQueryClient } from "@tanstack/react-query";

import { oauthQueryKeys } from "./oauth-query-keys";
import { useAdminEvent } from "@/shared/realtime";

export function useOAuthQuotaChangeEvent() {
  const queryClient = useQueryClient();
  useAdminEvent("oauth_quota_changed", true, () => {
    void queryClient.invalidateQueries({
      queryKey: oauthQueryKeys.quotas,
      refetchType: "active",
    });
  });
  useAdminEvent("oauth_refresh_diagnostic_changed", true, () => {
    void queryClient.invalidateQueries({
      queryKey: oauthQueryKeys.accounts,
      refetchType: "active",
    });
  });
}
