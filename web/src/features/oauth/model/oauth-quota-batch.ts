const MAX_CONCURRENT_OAUTH_QUOTA_REQUESTS = 6;

export type OAuthQuotaBatchOutcome<T> =
  | { accountId: string; status: "fulfilled"; value: T }
  | { accountId: string; status: "rejected"; reason: unknown };

/** Runs a full OAuth account collection with bounded, all-settled semantics. */
export async function runOAuthQuotaBatch<T>(
  accountIds: readonly string[],
  operation: (accountId: string) => Promise<T>,
): Promise<Array<OAuthQuotaBatchOutcome<T>>> {
  const outcomes = new Array<OAuthQuotaBatchOutcome<T>>(accountIds.length);
  let nextIndex = 0;
  const workers = Array.from(
    {
      length: Math.min(MAX_CONCURRENT_OAUTH_QUOTA_REQUESTS, accountIds.length),
    },
    async () => {
      while (nextIndex < accountIds.length) {
        const index = nextIndex;
        nextIndex += 1;
        const accountId = accountIds[index];
        try {
          outcomes[index] = {
            accountId,
            status: "fulfilled",
            value: await operation(accountId),
          };
        } catch (reason) {
          outcomes[index] = { accountId, status: "rejected", reason };
        }
      }
    },
  );
  await Promise.all(workers);
  return outcomes;
}
