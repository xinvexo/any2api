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

export function useOAuthAccountMutations() {
  const queryClient = useQueryClient();
  const publish = (next: OAuthAccountConfiguration) => {
    queryClient.setQueryData<OAuthAccountConfiguration>(
      oauthQueryKeys.accounts,
      (current) =>
        !current || next.configRevision >= current.configRevision ? next : current,
    );
  };
  const refreshAfterFailure = () =>
    queryClient.refetchQueries({ queryKey: oauthQueryKeys.accounts, type: "active" });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: OAuthAccountUpdateInput }) =>
      updateOAuthAccount(id, input),
    onSuccess: publish,
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
    onSuccess: publish,
    onError: refreshAfterFailure,
    retry: false,
  });

  return {
    update,
    models,
    remove,
    isPending: update.isPending || models.isPending || remove.isPending,
  };
}
