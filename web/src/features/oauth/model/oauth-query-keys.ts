const quotaRoot = ["oauth", "quota"] as const;
const quotaRefreshRoot = ["oauth", "quota-refresh"] as const;
const quotaResetRoot = ["oauth", "quota-reset"] as const;

export const oauthQueryKeys = {
  accounts: ["oauth", "accounts"] as const,
  quotas: quotaRoot,
  quota: (accountId: string) => [...quotaRoot, accountId] as const,
  quotaRefresh: (accountId: string) => [...quotaRefreshRoot, accountId] as const,
  quotaReset: (accountId: string) => [...quotaResetRoot, accountId] as const,
};
