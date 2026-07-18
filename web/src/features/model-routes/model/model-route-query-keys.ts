export const modelRouteQueryKeys = {
  all: ["model-routes"] as const,
  list: () => [...modelRouteQueryKeys.all, "list"] as const,
};
