export const overviewResourcesQueryKeys = {
  all: ["overview-resources"] as const,
  current: () => [...overviewResourcesQueryKeys.all, "current"] as const,
};
