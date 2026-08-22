import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import type {
  ProviderCredentialConfiguration,
  ProviderCredentialCreateInput,
  ProviderCredentialRotateInput,
} from "../api/provider-credential-contracts";
import type { ProviderCredentialMutationResponse } from "../api/provider-credential-mutation-contracts";
import {
  createProviderCredential,
  rotateProviderCredential,
} from "../api/provider-credential-api";
import { mergeProviderCredentialMutationResponse } from "./merge-provider-credential-mutation-response";
import { providerQueryKeys } from "./provider-query-keys";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";

export function useProviderSecretActions(endpointId: string) {
  const queryClient = useQueryClient();
  const cacheKey = providerQueryKeys.credentials(endpointId);
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<ProviderCredentialConfiguration>({
      cacheKey,
      invalidateKey: providerQueryKeys.list(),
      refreshKey: cacheKey,
      refreshAfterPublish: true,
    });
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<unknown>(null);

  async function run(
    action: () => Promise<ProviderCredentialMutationResponse>,
    waitForAuthoritativeConfiguration = false,
  ) {
    setPending(true);
    setError(null);
    try {
      const acknowledgement = await action();
      const merged = mergeProviderCredentialMutationResponse(
        queryClient.getQueryData<ProviderCredentialConfiguration>(cacheKey),
        acknowledgement,
      );
      if (merged) {
        publish(merged);
      } else {
        void queryClient.invalidateQueries({ queryKey: providerQueryKeys.list() });
        void queryClient.invalidateQueries({ queryKey: cacheKey, exact: true });
      }
      if (waitForAuthoritativeConfiguration) {
        await queryClient.refetchQueries(
          { queryKey: cacheKey, exact: true, type: "active" },
          { throwOnError: true },
        );
      }
      return acknowledgement;
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
      run(() => createProviderCredential(endpointId, input), true),
    rotate: (id: string, input: ProviderCredentialRotateInput) =>
      run(() => rotateProviderCredential(id, input)),
    pending,
    error,
    reset: () => setError(null),
  };
}
