import { useQuery } from "@tanstack/react-query";

import { getHealth } from "../api/get-health";

export function useHealth() {
  return useQuery({
    queryKey: ["system", "health"],
    queryFn: ({ signal }) => getHealth(signal),
    refetchInterval: 15_000,
  });
}
