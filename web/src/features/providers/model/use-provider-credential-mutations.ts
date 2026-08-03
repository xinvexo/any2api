import { useMutation } from "@tanstack/react-query";

import type {
  ProviderCredentialConfiguration,
  ProviderCredentialModelsInput,
  ProviderCredentialUpdateInput,
} from "../api/provider-credential-contracts";
import {
  deleteProviderCredential,
  updateProviderCredential,
  setProviderCredentialModels,
} from "../api/provider-credential-api";
import { providerQueryKeys } from "./provider-query-keys";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";

export function useProviderCredentialMutations(endpointId: string) {
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<ProviderCredentialConfiguration>({
      cacheKey: providerQueryKeys.credentials(endpointId),
      invalidateKey: providerQueryKeys.list(),
      refreshKey: providerQueryKeys.credentials(endpointId),
    });

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
  const models = useMutation({
    mutationFn: ({ id, input }: { id: string; input: ProviderCredentialModelsInput }) =>
      setProviderCredentialModels(id, input),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });

  return { update, remove, models, isPending: update.isPending || remove.isPending || models.isPending };
}
