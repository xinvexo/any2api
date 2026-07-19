import { useMutation, useQueryClient } from "@tanstack/react-query";

import { clearAllAffinity, clearCredentialAffinity } from "../api/affinity-api";
import { affinityQueryKeys } from "./affinity-query-keys";

export function useAffinityMutations() {
  const queryClient = useQueryClient();
  const refresh = () => queryClient.invalidateQueries({ queryKey: affinityQueryKeys.all });
  const clearAll = useMutation({
    mutationFn: clearAllAffinity,
    onSuccess: refresh,
    retry: false,
  });
  const clearCredential = useMutation({
    mutationFn: clearCredentialAffinity,
    onSuccess: refresh,
    retry: false,
  });

  return {
    clearAll,
    clearCredential,
    isPending: clearAll.isPending || clearCredential.isPending,
  };
}
