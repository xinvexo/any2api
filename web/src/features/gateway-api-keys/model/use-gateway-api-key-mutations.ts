import { useMutation, useQueryClient } from "@tanstack/react-query";

import type {
  GatewayApiKeyConfiguration,
  GatewayApiKeyCreateInput,
  GatewayApiKeyDeleteInput,
  GatewayApiKeyRotateInput,
  GatewayApiKeyUpdateInput,
} from "../api/gateway-api-key-contracts";
import {
  createGatewayApiKey,
  deleteGatewayApiKey,
  rotateGatewayApiKey,
  updateGatewayApiKey,
} from "../api/gateway-api-key-api";
import { selectNewestGatewayApiKeyConfiguration } from "./gateway-api-key-cache";
import { gatewayApiKeyQueryKeys } from "./gateway-api-key-query-keys";

export function useGatewayApiKeyMutations() {
  const queryClient = useQueryClient();
  const publish = (configuration: GatewayApiKeyConfiguration) => {
    queryClient.setQueryData<GatewayApiKeyConfiguration>(
      gatewayApiKeyQueryKeys.list(),
      (current) => selectNewestGatewayApiKeyConfiguration(current, configuration),
    );
    void queryClient.invalidateQueries({ queryKey: gatewayApiKeyQueryKeys.all });
  };
  const refreshAfterFailure = () => {
    void queryClient.refetchQueries({ queryKey: gatewayApiKeyQueryKeys.all, type: "active" });
  };
  const create = useMutation({
    mutationFn: (input: GatewayApiKeyCreateInput) => createGatewayApiKey(input),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: GatewayApiKeyUpdateInput }) =>
      updateGatewayApiKey(id, input),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const remove = useMutation({
    mutationFn: ({ id, input }: { id: string; input: GatewayApiKeyDeleteInput }) =>
      deleteGatewayApiKey(id, input),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const rotate = useMutation({
    mutationFn: ({ id, input }: { id: string; input: GatewayApiKeyRotateInput }) =>
      rotateGatewayApiKey(id, input),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  return {
    create,
    update,
    remove,
    rotate,
    isPending: create.isPending || update.isPending || remove.isPending || rotate.isPending,
  };
}
