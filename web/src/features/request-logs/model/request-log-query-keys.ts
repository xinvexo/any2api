export const requestLogQueryKeys = {
  all: ["request-logs"] as const,
  list: (limit: number) => [...requestLogQueryKeys.all, "list", limit] as const,
  detail: (requestId: string) => [...requestLogQueryKeys.all, "detail", requestId] as const,
};
