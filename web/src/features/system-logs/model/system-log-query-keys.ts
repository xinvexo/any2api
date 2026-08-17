export const systemLogQueryKeys = {
  all: ["system-logs"] as const,
  list: (showAdminOperations: boolean) =>
    [
      "system-logs",
      "list",
      showAdminOperations ? "with-admin" : "without-admin",
    ] as const,
  detail: (requestId: string) => ["system-logs", "detail", requestId] as const,
};
