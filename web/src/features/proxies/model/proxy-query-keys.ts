export const proxyQueryKeys = {
  all: ["proxies"] as const,
  list: () => [...proxyQueryKeys.all, "list"] as const,
};
