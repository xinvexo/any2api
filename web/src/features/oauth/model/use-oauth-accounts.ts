import { useQuery } from "@tanstack/react-query";

import { listOAuthAccounts } from "../api/oauth-api";
import { oauthQueryKeys } from "./oauth-query-keys";

export function useOAuthAccounts() {
  return useQuery({
    queryKey: oauthQueryKeys.accounts,
    queryFn: ({ signal }) => listOAuthAccounts(signal),
  });
}
