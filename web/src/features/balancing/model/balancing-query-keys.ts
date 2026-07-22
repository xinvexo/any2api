export const balancingQueryKeys = {
  all: ["balancing"] as const,
  runtime: () => [...balancingQueryKeys.all, "runtime"] as const,
};
