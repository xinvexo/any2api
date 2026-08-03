import { useRef, useState } from "react";

import {
  clearProxyAuthentication,
  setProxyAuthentication,
} from "../api/proxy-api";
import type {
  ProxyAuthenticationInput,
  ProxyConfiguration,
} from "../api/proxy-contracts";
import { proxyQueryKeys } from "./proxy-query-keys";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";

export function useProxyAuthenticationActions() {
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<ProxyConfiguration>({
      cacheKey: proxyQueryKeys.list(),
      invalidateKey: proxyQueryKeys.all,
      refreshKey: proxyQueryKeys.all,
    });
  const [pendingCount, setPendingCount] = useState(0);
  const [error, setError] = useState<unknown>(null);
  const generation = useRef(0);

  async function set(id: string, expectedRevision: number, input: ProxyAuthenticationInput) {
    const requestGeneration = ++generation.current;
    setPendingCount((count) => count + 1);
    setError(null);
    try {
      publish(await setProxyAuthentication(id, expectedRevision, input));
    } catch (nextError) {
      if (generation.current === requestGeneration) {
        setError(nextError);
      }
      void refreshAfterFailure();
      throw nextError;
    } finally {
      setPendingCount((count) => count - 1);
    }
  }

  async function clear(id: string, expectedRevision: number) {
    const requestGeneration = ++generation.current;
    setPendingCount((count) => count + 1);
    setError(null);
    try {
      publish(await clearProxyAuthentication(id, expectedRevision));
    } catch (nextError) {
      if (generation.current === requestGeneration) {
        setError(nextError);
      }
      void refreshAfterFailure();
      throw nextError;
    } finally {
      setPendingCount((count) => count - 1);
    }
  }

  return {
    set,
    clear,
    pending: pendingCount > 0,
    error,
    reset: () => {
      generation.current += 1;
      setError(null);
    },
  };
}
