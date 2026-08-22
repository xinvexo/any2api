import { useQuery, useQueryClient } from "@tanstack/react-query";

import { getBalancingRuntime } from "../api/balancing-api";
import { parseBalancingRuntime } from "../api/balancing-contracts";
import { useAdminEvent } from "@/shared/realtime";

export const balancingRuntimeQueryKey = ["balancing", "runtime"] as const;

export function useBalancingRuntime() {
  const queryClient = useQueryClient();
  useAdminEvent("overview_snapshot", true, (payload) => {
    const snapshot = record(payload);
    if (!snapshot || !("runtime" in snapshot)) {
      return;
    }
    try {
      queryClient.setQueryData(
        balancingRuntimeQueryKey,
        parseBalancingRuntime(snapshot.runtime),
      );
    } catch {
      // Ignore malformed realtime data; the HTTP query remains the fallback.
    }
  });

  return useQuery({
    queryKey: balancingRuntimeQueryKey,
    queryFn: ({ signal }) => getBalancingRuntime(signal),
  });
}

function record(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}
