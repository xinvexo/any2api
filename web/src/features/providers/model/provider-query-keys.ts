export const providerQueryKeys = {
  all: ["provider-endpoints"] as const,
  list: () => [...providerQueryKeys.all, "list"] as const,
  credentials: (endpointId: string) =>
    [...providerQueryKeys.all, "credentials", endpointId] as const,
};
