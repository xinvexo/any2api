import { useMutation } from "@tanstack/react-query";

import type { ProxyConfiguration, ProxyWriteInput } from "../api/proxy-contracts";
import { createProxy, deleteProxy, setGlobalProxy, updateProxy } from "../api/proxy-api";
import { proxyQueryKeys } from "./proxy-query-keys";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";

export function useProxyMutations() {
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<ProxyConfiguration>({
      cacheKey: proxyQueryKeys.list(),
      invalidateKey: proxyQueryKeys.all,
      refreshKey: proxyQueryKeys.all,
    });
  const refreshInBackground = () => void refreshAfterFailure();

  const create = useMutation({
    mutationFn: createProxy,
    onError: refreshInBackground,
    onSuccess: publish,
    retry: false,
  });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: ProxyWriteInput }) => updateProxy(id, input),
    onError: refreshInBackground,
    onSuccess: publish,
    retry: false,
  });
  const remove = useMutation({
    mutationFn: ({ id, expectedRevision }: { id: string; expectedRevision: number }) =>
      deleteProxy(id, expectedRevision),
    onError: refreshInBackground,
    onSuccess: publish,
    retry: false,
  });
  const setGlobal = useMutation({
    mutationFn: ({ id, expectedRevision }: { id: string; expectedRevision: number }) =>
      setGlobalProxy(id, expectedRevision),
    onError: refreshInBackground,
    onSuccess: publish,
    retry: false,
  });

  return {
    create,
    update,
    remove,
    setGlobal,
    isPending: create.isPending || update.isPending || remove.isPending || setGlobal.isPending,
  };
}
