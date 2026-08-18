import type { OAuthAccountConfiguration } from "../api/oauth-contracts";
import type { OAuthAccountMutationResponse } from "../api/oauth-account-mutation-contracts";

export function mergeOAuthAccountMutationResponse(
  current: OAuthAccountConfiguration | undefined,
  incoming: OAuthAccountMutationResponse,
): OAuthAccountConfiguration | undefined {
  if (!current) {
    return undefined;
  }
  if (incoming.configRevision < current.configRevision) {
    return current;
  }
  const currentById = new Map(current.items.map((account) => [account.id, account]));
  return {
    ...incoming,
    items: incoming.items.flatMap((account) => {
      const previous = currentById.get(account.id);
      if (!previous) {
        return [];
      }
      return [{
        ...account,
        availableModels: mergeModelCatalog(previous.availableModels, account.models),
        usage: previous.usage,
      }];
    }),
  };
}

function mergeModelCatalog(previous: string[], incoming: string[]) {
  const merged = [...previous];
  const known = new Set(previous);
  for (const model of incoming) {
    if (!known.has(model)) {
      known.add(model);
      merged.push(model);
    }
  }
  return merged;
}
