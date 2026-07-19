export const affinityQueryKeys = {
  all: ["affinity"] as const,
  runtime: () => [...affinityQueryKeys.all, "runtime"] as const,
};
