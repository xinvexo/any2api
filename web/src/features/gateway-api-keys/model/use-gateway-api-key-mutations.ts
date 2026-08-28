import { useMutation } from "@tanstack/react-query";
import { useState } from "react";

import type {
  GatewayApiKeyConfiguration,
  GatewayApiKeyDeleteInput,
  GatewayApiKeyRotateInput,
  GatewayApiKeySecretMutationResult,
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
  const create = useGatewayApiKeySecretMutation(createGatewayApiKey, publish, refreshAfterFailure);
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
  const rotate = useGatewayApiKeySecretMutation(
    ({ id, input }: { id: string; input: GatewayApiKeyRotateInput }) =>
      rotateGatewayApiKey(id, input),
    publish,
    refreshAfterFailure,
  );
  return {
    create,
    update,
    remove,
    rotate,
    isPending: create.isPending || update.isPending || remove.isPending || rotate.isPending,
  };
}

function useGatewayApiKeySecretMutation<TInput>(
  mutate: (input: TInput) => Promise<GatewayApiKeySecretMutationResult>,
  publish: (configuration: GatewayApiKeyConfiguration) => void,
  refreshAfterFailure: () => Promise<unknown>,
) {
  const [isPending, setPending] = useState(false);
  const [error, setError] = useState<unknown>(null);

  async function mutateAsync(input: TInput) {
    setPending(true);
    setError(null);
    try {
      const result = await mutate(input);
      publish(result.configuration);
      return result;
    } catch (nextError) {
      setError(nextError);
      void refreshAfterFailure().catch(() => undefined);
      throw nextError;
    } finally {
      setPending(false);
    }
  }

  return {
    mutateAsync,
    isPending,
    error,
    reset: () => setError(null),
  };
}
