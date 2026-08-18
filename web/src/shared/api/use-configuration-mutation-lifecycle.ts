import { useQueryClient, type QueryKey } from "@tanstack/react-query";

interface RevisionedConfiguration {
  configRevision: number;
}

interface ConfigurationMutationLifecycleOptions {
  cacheKey: QueryKey;
  invalidateKey?: QueryKey;
  refreshKey: QueryKey;
  refreshAfterPublish?: boolean;
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
  refreshAfterPublish = false,
}: ConfigurationMutationLifecycleOptions) {
  const queryClient = useQueryClient();

  function publish(configuration: T) {
    queryClient.setQueryData<T>(cacheKey, (current) =>
      selectNewestConfiguration(current, configuration),
    );
    if (invalidateKey) {
      void queryClient.invalidateQueries({ queryKey: invalidateKey });
    }
    if (refreshAfterPublish) {
      void queryClient.invalidateQueries({ queryKey: refreshKey, exact: true });
    }
  }

  function refreshAfterFailure() {
    return queryClient.refetchQueries({ queryKey: refreshKey, type: "active" });
  }

  return { publish, refreshAfterFailure };
}
