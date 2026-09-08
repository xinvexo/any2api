export const systemLogQueryKeys = {
  all: ["system-logs"] as const,
  list: (showAdminOperations: boolean, filters: SystemLogFilters = EMPTY_SYSTEM_LOG_FILTERS) =>
    [
      "system-logs",
      "list",
      showAdminOperations ? "with-admin" : "without-admin",
      filters,
    ] as const,
  detail: (requestId: string) => ["system-logs", "detail", requestId] as const,
};
import { EMPTY_SYSTEM_LOG_FILTERS, type SystemLogFilters } from "../api/system-log-contracts";
