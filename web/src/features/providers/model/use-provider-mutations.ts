import { useMutation } from "@tanstack/react-query";

import type {
  ProviderEndpointConfiguration,
  ProviderEndpointWriteInput,
} from "../api/provider-contracts";
import {
  createProviderEndpoint,
  deleteProviderEndpoint,
  updateProviderEndpoint,
} from "../api/provider-api";
import { providerQueryKeys } from "./provider-query-keys";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";

export function useProviderEndpointMutations() {
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<ProviderEndpointConfiguration>({
      cacheKey: providerQueryKeys.list(),
      invalidateKey: providerQueryKeys.all,
      refreshKey: providerQueryKeys.all,
    });

  const create = useMutation({
    mutationFn: createProviderEndpoint,
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: ProviderEndpointWriteInput }) =>
      updateProviderEndpoint(id, input),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const remove = useMutation({
    mutationFn: ({ id, expectedRevision }: { id: string; expectedRevision: number }) =>
      deleteProviderEndpoint(id, expectedRevision),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });

  return {
    create,
    update,
    remove,
    isPending: create.isPending || update.isPending || remove.isPending,
  };
}
