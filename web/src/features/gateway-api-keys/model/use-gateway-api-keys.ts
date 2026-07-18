import { useQuery } from "@tanstack/react-query";

import { listGatewayApiKeys } from "../api/gateway-api-key-api";
import { gatewayApiKeyQueryKeys } from "./gateway-api-key-query-keys";

export function useGatewayApiKeys() {
  return useQuery({
    queryKey: gatewayApiKeyQueryKeys.list(),
    queryFn: ({ signal }) => listGatewayApiKeys(signal),
  });
}
