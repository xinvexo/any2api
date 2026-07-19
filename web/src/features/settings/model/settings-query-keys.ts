export const settingsQueryKeys = {
  all: ["settings"] as const,
  list: () => [...settingsQueryKeys.all, "list"] as const,
};
