import type { QueryClient } from "@tanstack/react-query";

import type {
  OAuthAccount,
  OAuthAccountConfiguration,
} from "../api/oauth-contracts";
import { deleteOAuthAccount, listOAuthAccounts } from "../api/oauth-api";
import { ApiError } from "@/shared/api/http-client";

import { oauthQueryKeys } from "./oauth-query-keys";
import { refreshOAuthAccountQuota } from "./oauth-quota-query";
import { runOAuthQuotaBatch } from "./oauth-quota-batch";

export interface InvalidOAuthAccountCandidate {
  id: string;
  label: string;
  tokenVersion: number;
}

export interface InvalidOAuthAccountInspection {
  total: number;
  inconclusive: number;
  candidates: InvalidOAuthAccountCandidate[];
}

export interface InvalidOAuthAccountDeletion {
  requested: number;
  deleted: number;
  skipped: number;
  failed: number;
}

export async function inspectInvalidOAuthAccounts(
  queryClient: QueryClient,
  accountIds: readonly string[],
): Promise<InvalidOAuthAccountInspection> {
  const outcomes = await runOAuthQuotaBatch(accountIds, (accountId) =>
    refreshOAuthAccountQuota(queryClient, accountId),
  );
  const invalidTokenVersions = new Map(
    outcomes.flatMap((outcome) => {
      if (outcome.status !== "rejected") {
        return [];
      }
      const tokenVersion = invalidOAuthAuthenticationTokenVersion(outcome.reason);
      return tokenVersion === null ? [] : [[outcome.accountId, tokenVersion] as const];
    }),
  );
  const inconclusive = outcomes.filter(
    (outcome) =>
      outcome.status === "rejected" &&
      !isInvalidOAuthAuthenticationError(outcome.reason),
  ).length;
  const current = await reloadOAuthAccounts(queryClient);
  return {
    total: accountIds.length,
    inconclusive,
    candidates: current.items
      .filter(
        (account) => invalidTokenVersions.get(account.id) === account.tokenVersion,
      )
      .map(candidateFromAccount),
  };
}

export async function deleteInspectedOAuthAccounts(
  queryClient: QueryClient,
  candidates: readonly InvalidOAuthAccountCandidate[],
): Promise<InvalidOAuthAccountDeletion> {
  const result: InvalidOAuthAccountDeletion = {
    requested: candidates.length,
    deleted: 0,
    skipped: 0,
    failed: 0,
  };
  let current = await reloadOAuthAccounts(queryClient);

  for (let index = 0; index < candidates.length; index += 1) {
    const candidate = candidates[index];
    const account = matchingAccount(current, candidate);
    if (!account) {
      result.skipped += 1;
      await removeQuotaCache(queryClient, candidate.id);
      continue;
    }

    try {
      current = await deleteOAuthAccount(
        account.id,
        current.configRevision,
        account.configVersion,
      );
      publishOAuthAccounts(queryClient, current);
      await removeQuotaCache(queryClient, account.id);
      result.deleted += 1;
    } catch (error) {
      const reloaded = await tryReloadOAuthAccounts(queryClient);
      if (!reloaded) {
        result.failed += candidates.length - index;
        break;
      }
      current = reloaded;
      const latest = matchingAccount(current, candidate);
      if (!latest) {
        const stillExists = current.items.some((item) => item.id === candidate.id);
        result[stillExists ? "skipped" : "deleted"] += 1;
        await removeQuotaCache(queryClient, candidate.id);
        continue;
      }
      if (!isOAuthAccountVersionConflict(error)) {
        result.failed += 1;
        continue;
      }
      try {
        current = await deleteOAuthAccount(
          latest.id,
          current.configRevision,
          latest.configVersion,
        );
        publishOAuthAccounts(queryClient, current);
        await removeQuotaCache(queryClient, latest.id);
        result.deleted += 1;
      } catch {
        const next = await tryReloadOAuthAccounts(queryClient);
        if (!next) {
          result.failed += candidates.length - index;
          return result;
        }
        current = next;
        const stillExists = current.items.some((item) => item.id === candidate.id);
        const stillMatches = matchingAccount(current, candidate);
        if (!stillExists) {
          result.deleted += 1;
          await removeQuotaCache(queryClient, candidate.id);
        } else if (!stillMatches) {
          result.skipped += 1;
          await removeQuotaCache(queryClient, candidate.id);
        } else {
          result.failed += 1;
        }
      }
    }
  }
  return result;
}

export function isInvalidOAuthAuthenticationError(error: unknown): boolean {
  return invalidOAuthAuthenticationTokenVersion(error) !== null;
}

function invalidOAuthAuthenticationTokenVersion(error: unknown): number | null {
  if (!(error instanceof ApiError) || error.diagnostic?.reauthorizationRequired !== true) {
    return null;
  }
  const reason = error.diagnostic.reason;
  switch (error.code) {
    case "oauth_refresh_token_missing":
      return reason === "refresh_token_missing" ? error.diagnostic.tokenVersion : null;
    case "oauth_refresh_permanently_rejected":
      return [
        "invalid_grant",
        "refresh_token_expired",
        "refresh_token_reused",
        "refresh_token_invalidated",
      ].includes(reason)
        ? error.diagnostic.tokenVersion
        : null;
    case "oauth_refreshed_access_token_rejected":
      return reason === "refreshed_access_token_rejected"
        ? error.diagnostic.tokenVersion
        : null;
    default:
      return null;
  }
}

function candidateFromAccount(account: OAuthAccount): InvalidOAuthAccountCandidate {
  return {
    id: account.id,
    label: account.label,
    tokenVersion: account.tokenVersion,
  };
}

function matchingAccount(
  configuration: OAuthAccountConfiguration,
  candidate: InvalidOAuthAccountCandidate,
) {
  const account = configuration.items.find((item) => item.id === candidate.id);
  return account?.tokenVersion === candidate.tokenVersion ? account : null;
}

function isOAuthAccountVersionConflict(error: unknown) {
  return (
    error instanceof ApiError &&
    (error.code === "revision_conflict" ||
      error.code === "oauth_account_version_conflict")
  );
}

async function reloadOAuthAccounts(queryClient: QueryClient) {
  const configuration = await listOAuthAccounts();
  publishOAuthAccounts(queryClient, configuration);
  return configuration;
}

async function tryReloadOAuthAccounts(queryClient: QueryClient) {
  try {
    return await reloadOAuthAccounts(queryClient);
  } catch {
    return null;
  }
}

function publishOAuthAccounts(
  queryClient: QueryClient,
  next: OAuthAccountConfiguration,
) {
  queryClient.setQueryData<OAuthAccountConfiguration>(
    oauthQueryKeys.accounts,
    (current) =>
      !current || next.configRevision >= current.configRevision ? next : current,
  );
}

async function removeQuotaCache(queryClient: QueryClient, accountId: string) {
  const queryKey = oauthQueryKeys.quota(accountId);
  await queryClient.cancelQueries({ queryKey, exact: true });
  queryClient.removeQueries({ queryKey, exact: true });
}
