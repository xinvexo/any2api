import { useQuery } from "@tanstack/react-query";

import { getAffinity } from "../api/affinity-api";
import { affinityQueryKeys } from "./affinity-query-keys";

export function useAffinity() {
  return useQuery({
    queryKey: affinityQueryKeys.runtime(),
    queryFn: ({ signal }) => getAffinity(100, signal),
    refetchInterval: 5_000,
  });
}
