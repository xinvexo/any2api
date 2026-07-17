import { useMutation, useQueryClient } from "@tanstack/react-query";

import type { ProxyConfiguration, ProxyWriteInput } from "../api/proxy-contracts";
import { createProxy, deleteProxy, setGlobalProxy, updateProxy } from "../api/proxy-api";
import { selectNewestProxyConfiguration } from "./proxy-cache";
import { proxyQueryKeys } from "./proxy-query-keys";

export function useProxyMutations() {
  const queryClient = useQueryClient();
  const publish = (configuration: ProxyConfiguration) => {
    queryClient.setQueryData<ProxyConfiguration>(proxyQueryKeys.list(), (current) =>
      selectNewestProxyConfiguration(current, configuration),
    );
    void queryClient.invalidateQueries({ queryKey: proxyQueryKeys.all });
  };
  const refreshAfterFailure = () => {
    void queryClient.refetchQueries({ queryKey: proxyQueryKeys.all, type: "active" });
  };

  const create = useMutation({
    mutationFn: createProxy,
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const update = useMutation({
    mutationFn: ({ id, input }: { id: string; input: ProxyWriteInput }) => updateProxy(id, input),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const remove = useMutation({
    mutationFn: ({ id, expectedRevision }: { id: string; expectedRevision: number }) =>
      deleteProxy(id, expectedRevision),
    onError: refreshAfterFailure,
    onSuccess: publish,
    retry: false,
  });
  const setGlobal = useMutation({
    mutationFn: ({ id, expectedRevision }: { id: string; expectedRevision: number }) =>
      setGlobalProxy(id, expectedRevision),
    onError: refreshAfterFailure,
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
