import type { GatewayApiKeyConfiguration } from "../api/gateway-api-key-contracts";

export function selectNewestGatewayApiKeyConfiguration(
  current: GatewayApiKeyConfiguration | undefined,
  next: GatewayApiKeyConfiguration,
) {
  return !current || next.configRevision >= current.configRevision ? next : current;
}
