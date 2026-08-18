import { useMutation, useQueryClient } from "@tanstack/react-query";

import type {
  ProviderCredentialConfiguration,
  ProviderCredentialModelsInput,
  ProviderCredentialUpdateInput,
} from "../api/provider-credential-contracts";
import type { ProviderCredentialMutationResponse } from "../api/provider-credential-mutation-contracts";
import {
  deleteProviderCredential,
  updateProviderCredential,
  setProviderCredentialModels,
} from "../api/provider-credential-api";
import { mergeProviderCredentialMutationResponse } from "./merge-provider-credential-mutation-response";
import { providerQueryKeys } from "./provider-query-keys";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";

export function useProviderCredentialMutations(endpointId: string) {
  const queryClient = useQueryClient();
  const cacheKey = providerQueryKeys.credentials(endpointId);
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<ProviderCredentialConfiguration>({
      cacheKey,
      invalidateKey: providerQueryKeys.list(),
      refreshKey: cacheKey,
      refreshAfterPublish: true,
    });

  function publishAcknowledgement(acknowledgement: ProviderCredentialMutationResponse) {
    const merged = mergeProviderCredentialMutationResponse(
      queryClient.getQueryData<ProviderCredentialConfiguration>(cacheKey),
      acknowledgement,
    );
    if (merged) {
      publish(merged);
      return;
    }
    void queryClient.invalidateQueries({ queryKey: providerQueryKeys.list() });
    void queryClient.invalidateQueries({ queryKey: cacheKey, exact: true });
  }

  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: ProviderCredentialUpdateInput }) =>
      updateProviderCredential(id, input),
    onError: refreshAfterFailure,
    onSuccess: publishAcknowledgement,
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
    onSuccess: publishAcknowledgement,
    retry: false,
  });
  const models = useMutation({
    mutationFn: ({ id, input }: { id: string; input: ProviderCredentialModelsInput }) =>
      setProviderCredentialModels(id, input),
    onError: refreshAfterFailure,
    onSuccess: publishAcknowledgement,
    retry: false,
  });

  return { update, remove, models, isPending: update.isPending || remove.isPending || models.isPending };
}
