import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import type {
  ProviderCredentialConfiguration,
  ProviderCredentialCreateInput,
  ProviderCredentialRotateInput,
} from "../api/provider-credential-contracts";
import {
  createProviderCredential,
  rotateProviderCredential,
} from "../api/provider-credential-api";
import { selectNewestCredentialConfiguration } from "./provider-credential-cache";
import { providerQueryKeys } from "./provider-query-keys";

export function useProviderSecretActions(endpointId: string) {
  const queryClient = useQueryClient();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<unknown>(null);

  const publish = (configuration: ProviderCredentialConfiguration) => {
    queryClient.setQueryData<ProviderCredentialConfiguration>(
      providerQueryKeys.credentials(endpointId),
      (current) => selectNewestCredentialConfiguration(current, configuration),
    );
    void queryClient.invalidateQueries({ queryKey: providerQueryKeys.list() });
  };

  async function run(action: () => Promise<ProviderCredentialConfiguration>) {
    setPending(true);
    setError(null);
    try {
      const configuration = await action();
      publish(configuration);
      return configuration;
    } catch (nextError) {
      setError(nextError);
      await queryClient.refetchQueries({
        queryKey: providerQueryKeys.credentials(endpointId),
        type: "active",
      });
      throw nextError;
    } finally {
      setPending(false);
    }
  }

  return {
    create: (input: ProviderCredentialCreateInput) =>
      run(() => createProviderCredential(endpointId, input)),
    rotate: (id: string, input: ProviderCredentialRotateInput) =>
      run(() => rotateProviderCredential(id, input)),
    pending,
    error,
    reset: () => setError(null),
  };
}
