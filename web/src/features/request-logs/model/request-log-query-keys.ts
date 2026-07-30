export const requestLogQueryKeys = {
  all: ["request-logs"] as const,
  list: (page: number, pageSize: number) =>
    [...requestLogQueryKeys.all, "list", page, pageSize] as const,
  detail: (requestId: string) => [...requestLogQueryKeys.all, "detail", requestId] as const,
};
