import { useMutation, useQueryClient } from "@tanstack/react-query";

import type {
  OAuthAccountConfiguration,
  OAuthAccountModelsInput,
  OAuthAccountUpdateInput,
} from "../api/oauth-contracts";
import {
  deleteOAuthAccount,
  setOAuthAccountModels,
  updateOAuthAccount,
} from "../api/oauth-api";
import { oauthQueryKeys } from "./oauth-query-keys";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";

export function useOAuthAccountMutations() {
  const queryClient = useQueryClient();
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<OAuthAccountConfiguration>({
      cacheKey: oauthQueryKeys.accounts,
      refreshKey: oauthQueryKeys.accounts,
    });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: OAuthAccountUpdateInput }) =>
      updateOAuthAccount(id, input),
    onSuccess: publish,
    onError: refreshAfterFailure,
    retry: false,
  });
  const remove = useMutation({
    mutationFn: ({
      id,
      expectedRevision,
      expectedConfigVersion,
    }: {
      id: string;
      expectedRevision: number;
      expectedConfigVersion: number;
    }) => deleteOAuthAccount(id, expectedRevision, expectedConfigVersion),
    onMutate: async () => {
      await queryClient.cancelQueries({ queryKey: oauthQueryKeys.quotas });
    },
    onSuccess: async (next, { id }) => {
      publish(next);
      const queryKey = oauthQueryKeys.quota(id);
      await queryClient.cancelQueries({ queryKey, exact: true });
      queryClient.removeQueries({ queryKey, exact: true });
    },
    onError: refreshAfterFailure,
    retry: false,
  });
  const models = useMutation({
    mutationFn: ({ id, input }: { id: string; input: OAuthAccountModelsInput }) =>
      setOAuthAccountModels(id, input),
    onSuccess: publish,
    onError: refreshAfterFailure,
    retry: false,
  });

  return {
    update,
    remove,
    models,
    isPending: update.isPending || remove.isPending || models.isPending,
  };
}
