import { useQuery } from "@tanstack/react-query";

import { getAffinity } from "../api/affinity-api";
import { affinityQueryKeys } from "./affinity-query-keys";

export function useAffinity() {
  return useQuery({
    queryKey: affinityQueryKeys.runtime(),
    queryFn: ({ signal }) => getAffinity(signal),
    refetchInterval: 15_000,
  });
}
