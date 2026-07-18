import type { ProviderEndpointConfiguration } from "../api/provider-contracts";

export function selectNewestProviderConfiguration(
  current: ProviderEndpointConfiguration | undefined,
  incoming: ProviderEndpointConfiguration,
) {
  if (!current || incoming.configRevision >= current.configRevision) {
    return incoming;
  }
  return current;
}
