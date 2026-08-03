import { useQueryClient, type QueryKey } from "@tanstack/react-query";

interface RevisionedConfiguration {
  configRevision: number;
}

interface ConfigurationMutationLifecycleOptions {
  cacheKey: QueryKey;
  invalidateKey?: QueryKey;
  refreshKey: QueryKey;
}

export function selectNewestConfiguration<T extends RevisionedConfiguration>(
  current: T | undefined,
  incoming: T,
) {
  return !current || incoming.configRevision >= current.configRevision ? incoming : current;
}

export function useConfigurationMutationLifecycle<T extends RevisionedConfiguration>({
  cacheKey,
  invalidateKey,
  refreshKey,
}: ConfigurationMutationLifecycleOptions) {
  const queryClient = useQueryClient();

  function publish(configuration: T) {
    queryClient.setQueryData<T>(cacheKey, (current) =>
      selectNewestConfiguration(current, configuration),
    );
    if (invalidateKey) {
      void queryClient.invalidateQueries({ queryKey: invalidateKey });
    }
  }

  function refreshAfterFailure() {
    return queryClient.refetchQueries({ queryKey: refreshKey, type: "active" });
  }

  return { publish, refreshAfterFailure };
}
