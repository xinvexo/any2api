import { useState } from "react";

import type {
  ProviderCredentialConfiguration,
  ProviderCredentialCreateInput,
  ProviderCredentialRotateInput,
} from "../api/provider-credential-contracts";
import {
  createProviderCredential,
  rotateProviderCredential,
} from "../api/provider-credential-api";
import { providerQueryKeys } from "./provider-query-keys";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";

export function useProviderSecretActions(endpointId: string) {
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<ProviderCredentialConfiguration>({
      cacheKey: providerQueryKeys.credentials(endpointId),
      invalidateKey: providerQueryKeys.list(),
      refreshKey: providerQueryKeys.credentials(endpointId),
    });
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<unknown>(null);

  async function run(action: () => Promise<ProviderCredentialConfiguration>) {
    setPending(true);
    setError(null);
    try {
      const configuration = await action();
      publish(configuration);
      return configuration;
    } catch (nextError) {
      setError(nextError);
      await refreshAfterFailure();
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
