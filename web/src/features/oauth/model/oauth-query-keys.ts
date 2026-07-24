const quotaRoot = ["oauth", "quota"] as const;
const quotaResetRoot = ["oauth", "quota-reset"] as const;

export const oauthQueryKeys = {
  accounts: ["oauth", "accounts"] as const,
  quotas: quotaRoot,
  quota: (accountId: string) => [...quotaRoot, accountId] as const,
  quotaReset: (accountId: string) => [...quotaResetRoot, accountId] as const,
};
