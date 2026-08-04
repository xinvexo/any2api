export const requestLogQueryKeys = {
  all: ["request-logs"] as const,
  list: (cursor: string | null, pageSize: number) =>
    [...requestLogQueryKeys.all, "list", cursor ?? "latest", pageSize] as const,
  detail: (requestId: string) => [...requestLogQueryKeys.all, "detail", requestId] as const,
};
