import { useQuery } from "@tanstack/react-query";

import { getBalancingRuntime } from "../api/balancing-api";
import { balancingQueryKeys } from "./balancing-query-keys";

export function useBalancingRuntime() {
  return useQuery({
    queryKey: balancingQueryKeys.runtime(),
    queryFn: ({ signal }) => getBalancingRuntime(signal),
    refetchInterval: 5_000,
  });
}
