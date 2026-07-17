import { useQuery } from "@tanstack/react-query";

import { listProxies } from "../api/proxy-api";
import { proxyQueryKeys } from "./proxy-query-keys";

export function useProxies() {
  return useQuery({
    queryKey: proxyQueryKeys.list(),
    queryFn: ({ signal }) => listProxies(signal),
  });
}
