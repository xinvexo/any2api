import { useMutation, useQueryClient } from "@tanstack/react-query";

import { clearSystemLogs } from "../api/system-log-api";
import { advanceSystemLogFeedGeneration } from "./system-log-feed-generation";
import { systemLogQueryKeys } from "./system-log-query-keys";

export function useClearSystemLogs() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: clearSystemLogs,
    onMutate: async () => {
      advanceSystemLogFeedGeneration();
      await queryClient.cancelQueries({ queryKey: systemLogQueryKeys.all });
    },
    onSuccess: () => {
      queryClient.removeQueries({ queryKey: systemLogQueryKeys.all });
    },
    onSettled: () => {
      advanceSystemLogFeedGeneration();
    },
  });
}
