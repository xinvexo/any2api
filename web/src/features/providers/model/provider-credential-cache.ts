import type { ProviderCredentialConfiguration } from "../api/provider-credential-contracts";

export function selectNewestCredentialConfiguration(
  current: ProviderCredentialConfiguration | undefined,
  incoming: ProviderCredentialConfiguration,
) {
  if (!current || incoming.configRevision >= current.configRevision) {
    return incoming;
  }
  return current;
}
