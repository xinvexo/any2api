import { useMutation, useQueryClient } from "@tanstack/react-query";

import type {
  ProviderCredentialConfiguration,
  ProviderCredentialUpdateInput,
} from "../api/provider-credential-contracts";
import {
  deleteProviderCredential,
  updateProviderCredential,
} from "../api/provider-credential-api";
import { selectNewestCredentialConfiguration } from "./provider-credential-cache";
import { providerQueryKeys } from "./provider-query-keys";

export function useProviderCredentialMutations(endpointId: string) {
  const queryClient = useQueryClient();
  const publish = (configuration: ProviderCredentialConfiguration) => {
    queryClient.setQueryData<ProviderCredentialConfiguration>(
      providerQueryKeys.credentials(endpointId),
      (current) => selectNewestCredentialConfiguration(current, configuration),
    );
    void queryClient.invalidateQueries({ queryKey: providerQueryKeys.list() });
  };
  const refreshAfterFailure = async () => {
    await queryClient.refetchQueries({
      queryKey: providerQueryKeys.credentials(endpointId),
      type: "active",
    });
  };

  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: ProviderCredentialUpdateInput }) =>
      updateProviderCredential(id, input),
    onError: refreshAfterFailure,
    onSuccess: publish,
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
    }) => deleteProviderCredential(id, expectedRevision, expectedConfigVersion),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });

  return { update, remove, isPending: update.isPending || remove.isPending };
}
