import { useMutation, useQueryClient } from "@tanstack/react-query";

import type {
  GatewayApiKeyConfiguration,
  GatewayApiKeyRevokeInput,
  GatewayApiKeyUpdateInput,
} from "../api/gateway-api-key-contracts";
import { revokeGatewayApiKey, updateGatewayApiKey } from "../api/gateway-api-key-api";
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
  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: GatewayApiKeyUpdateInput }) =>
      updateGatewayApiKey(id, input),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const revoke = useMutation({
    mutationFn: ({ id, input }: { id: string; input: GatewayApiKeyRevokeInput }) =>
      revokeGatewayApiKey(id, input),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  return {
    update,
    revoke,
    isPending: update.isPending || revoke.isPending,
  };
}
