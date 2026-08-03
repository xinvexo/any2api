import { useMutation } from "@tanstack/react-query";

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
import { gatewayApiKeyQueryKeys } from "./gateway-api-key-query-keys";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";

export function useGatewayApiKeyMutations() {
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<GatewayApiKeyConfiguration>({
      cacheKey: gatewayApiKeyQueryKeys.list(),
      invalidateKey: gatewayApiKeyQueryKeys.all,
      refreshKey: gatewayApiKeyQueryKeys.all,
    });
  const refreshInBackground = () => void refreshAfterFailure();
  const create = useMutation({
    mutationFn: (input: GatewayApiKeyCreateInput) => createGatewayApiKey(input),
    onError: refreshInBackground,
    onSuccess: publish,
    retry: false,
  });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: GatewayApiKeyUpdateInput }) =>
      updateGatewayApiKey(id, input),
    onError: refreshInBackground,
    onSuccess: publish,
    retry: false,
  });
  const remove = useMutation({
    mutationFn: ({ id, input }: { id: string; input: GatewayApiKeyDeleteInput }) =>
      deleteGatewayApiKey(id, input),
    onError: refreshInBackground,
    onSuccess: publish,
    retry: false,
  });
  const rotate = useMutation({
    mutationFn: ({ id, input }: { id: string; input: GatewayApiKeyRotateInput }) =>
      rotateGatewayApiKey(id, input),
    onError: refreshInBackground,
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
