import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import type {
  GatewayApiKeyConfiguration,
  GatewayApiKeyCreateInput,
  GatewayApiKeyRotateInput,
  GatewayApiKeySecretReceipt,
} from "../api/gateway-api-key-contracts";
import { createGatewayApiKey, rotateGatewayApiKey } from "../api/gateway-api-key-api";
import { selectNewestGatewayApiKeyConfiguration } from "./gateway-api-key-cache";
import { gatewayApiKeyQueryKeys } from "./gateway-api-key-query-keys";

export function useGatewayApiKeySecretActions() {
  const queryClient = useQueryClient();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<unknown>(null);

  const publish = (configuration: GatewayApiKeyConfiguration) => {
    queryClient.setQueryData<GatewayApiKeyConfiguration>(
      gatewayApiKeyQueryKeys.list(),
      (current) => selectNewestGatewayApiKeyConfiguration(current, configuration),
    );
    void queryClient.invalidateQueries({ queryKey: gatewayApiKeyQueryKeys.all });
  };

  async function run(action: () => Promise<GatewayApiKeySecretReceipt>) {
    setPending(true);
    setError(null);
    try {
      const receipt = await action();
      publish(receipt.configuration);
      return receipt;
    } catch (nextError) {
      setError(nextError);
      await queryClient.refetchQueries({
        queryKey: gatewayApiKeyQueryKeys.list(),
        type: "active",
      });
      throw nextError;
    } finally {
      setPending(false);
    }
  }

  return {
    create: (input: GatewayApiKeyCreateInput) => run(() => createGatewayApiKey(input)),
    regenerate: (id: string, input: GatewayApiKeyRotateInput) =>
      run(() => rotateGatewayApiKey(id, input)),
    pending,
    error,
    reset: () => setError(null),
  };
}
