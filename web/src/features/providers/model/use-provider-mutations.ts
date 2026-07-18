import { useMutation, useQueryClient } from "@tanstack/react-query";

import type {
  ProviderEndpointConfiguration,
  ProviderEndpointWriteInput,
} from "../api/provider-contracts";
import {
  createProviderEndpoint,
  deleteProviderEndpoint,
  updateProviderEndpoint,
} from "../api/provider-api";
import { selectNewestProviderConfiguration } from "./provider-cache";
import { providerQueryKeys } from "./provider-query-keys";

export function useProviderEndpointMutations() {
  const queryClient = useQueryClient();
  const publish = (configuration: ProviderEndpointConfiguration) => {
    queryClient.setQueryData<ProviderEndpointConfiguration>(providerQueryKeys.list(), (current) =>
      selectNewestProviderConfiguration(current, configuration),
    );
    void queryClient.invalidateQueries({ queryKey: providerQueryKeys.all });
  };
  const refreshAfterFailure = async () => {
    await queryClient.refetchQueries({ queryKey: providerQueryKeys.all, type: "active" });
  };

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
