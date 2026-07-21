import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import {
  clearProxyAuthentication,
  setProxyAuthentication,
} from "../api/proxy-api";
import type {
  ProxyAuthenticationInput,
  ProxyConfiguration,
} from "../api/proxy-contracts";
import { selectNewestProxyConfiguration } from "./proxy-cache";
import { proxyQueryKeys } from "./proxy-query-keys";

export function useProxyAuthenticationActions() {
  const queryClient = useQueryClient();
  const [pendingCount, setPendingCount] = useState(0);
  const [error, setError] = useState<unknown>(null);
  const generation = useRef(0);

  function publish(configuration: ProxyConfiguration) {
    queryClient.setQueryData<ProxyConfiguration>(proxyQueryKeys.list(), (current) =>
      selectNewestProxyConfiguration(current, configuration),
    );
    void queryClient.invalidateQueries({ queryKey: proxyQueryKeys.all });
  }

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
      void queryClient.refetchQueries({ queryKey: proxyQueryKeys.all, type: "active" });
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
      void queryClient.refetchQueries({ queryKey: proxyQueryKeys.all, type: "active" });
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
