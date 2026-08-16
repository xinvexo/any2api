import { useQuery } from "@tanstack/react-query";

import { getBalancingRuntime } from "../api/balancing-api";
import { balancingQueryKeys } from "./balancing-query-keys";

export const BALANCING_RUNTIME_REFRESH_INTERVAL_MS = 5_000;

export function useBalancingRuntime() {
  return useQuery({
    queryKey: balancingQueryKeys.runtime(),
    queryFn: ({ signal }) => getBalancingRuntime(signal),
    refetchInterval: BALANCING_RUNTIME_REFRESH_INTERVAL_MS,
  });
}
