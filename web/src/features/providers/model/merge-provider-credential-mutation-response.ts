import type { ProviderCredentialConfiguration } from "../api/provider-credential-contracts";
import type { ProviderCredentialMutationResponse } from "../api/provider-credential-mutation-contracts";

export function mergeProviderCredentialMutationResponse(
  current: ProviderCredentialConfiguration | undefined,
  incoming: ProviderCredentialMutationResponse,
): ProviderCredentialConfiguration | undefined {
  if (!current) {
    return undefined;
  }
  if (incoming.configRevision < current.configRevision) {
    return current;
  }
  const currentById = new Map(current.items.map((credential) => [credential.id, credential]));
  return {
    ...incoming,
    items: incoming.items.flatMap((credential) => {
      const previous = currentById.get(credential.id);
      return previous ? [{ ...credential, usage: previous.usage }] : [];
    }),
  };
}
