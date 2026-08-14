import type { OAuthAccount } from "../api/oauth-contracts";
import type { OAuthQuotaSnapshot } from "../api/oauth-quota-contracts";

/** Neutral chip (plan tier, region, …), success, or warning runtime state. */
type OAuthAccountBadgeTone = "neutral" | "success" | "warning" | "danger";

interface OAuthAccountBadge {
  key: string;
  label: string;
  tone: OAuthAccountBadgeTone;
}

interface OAuthAccountMetric {
  key: string;
  label: string;
  value: string;
  title?: string;
  tone?: "success" | "warning";
}

/**
 * Provider-agnostic view model for OAuth account cards and drawers.
 * Providers only differ by how fields map into badges/metrics —
 * the card chrome stays the same.
 */
export interface OAuthAccountPresentation {
  id: string;
  title: string;
  subtitle: string;
  enabled: boolean;
  badges: OAuthAccountBadge[];
  metrics: OAuthAccountMetric[];
  modelCatalog: string[];
}

export function presentOAuthAccount(
  account: OAuthAccount,
  quota: OAuthQuotaSnapshot | null = null,
  nowSeconds: number = Math.floor(Date.now() / 1_000),
): OAuthAccountPresentation {
  const badges: OAuthAccountBadge[] = [];

  const planType = quota?.subscriptionTier ?? account.planType;
  if (planType) {
    badges.push({ key: "plan", label: planType, tone: "neutral" });
  }
  badges.push(describeAccountStatus(account, quota, nowSeconds));
  if (account.botFlagged === true) {
    badges.push({ key: "bot-flagged", label: "机器人账号", tone: "warning" });
  }

  const metrics: OAuthAccountMetric[] = [
    {
      key: "rpm",
      label: "60s RPM",
      value:
        account.runtime.rpm60s.limit === null
          ? "无限制"
          : `${account.runtime.rpm60s.used} / ${account.runtime.rpm60s.limit}`,
    },
    {
      key: "in-flight",
      label: "处理中",
      value: String(account.runtime.inFlight),
    },
    {
      key: "models",
      label: "模型",
      value: String(account.availableModels.length),
    },
    {
      key: "expires",
      label: "过期",
      value: formatExpiry(account.expiresAt),
      title: formatExpiry(account.expiresAt),
    },
  ];

  return {
    id: account.id,
    title: account.label,
    subtitle: account.safeAccountEmail ?? "未提供邮箱",
    enabled: account.enabled,
    badges,
    metrics,
    modelCatalog: [...account.availableModels].sort((left, right) =>
      left.localeCompare(right),
    ),
  };
}

function describeAccountStatus(
  account: OAuthAccount,
  quota: OAuthQuotaSnapshot | null,
  nowSeconds: number,
): OAuthAccountBadge {
  const expired = account.expiresAt !== null && account.expiresAt <= nowSeconds;
  if (
    account.tokenRefreshFailure?.reauthorizationRequired
    || expired
    || account.runtime.status === "authentication_expired"
  ) {
    return {
      key: account.tokenRefreshFailure?.reauthorizationRequired
        ? "token-refresh-failed"
        : "runtime-status",
      label: "过期",
      tone: "danger",
    };
  }
  if (quota?.accountStatus?.userBlockedReason) {
    return { key: "upstream-restricted", label: "受限", tone: "warning" };
  }
  if (quotaIsExhausted(quota)) {
    return { key: "quota-exhausted", label: "耗尽", tone: "warning" };
  }
  if (account.tokenRefreshFailure) {
    return { key: "token-refresh-failed", label: "刷新异常", tone: "warning" };
  }
  return { key: "runtime-status", ...describeRuntimeStatus(account.runtime.status) };
}

function describeRuntimeStatus(status: OAuthAccount["runtime"]["status"]): {
  label: string;
  tone: "success" | "warning" | "danger";
} {
  switch (status) {
    case "ready":
      return { label: "正常", tone: "success" };
    case "disabled":
    case "endpoint_disabled":
      return { label: "停用", tone: "warning" };
    case "authentication_expired":
      return { label: "过期", tone: "danger" };
    case "rate_limited":
      return { label: "RPM 用尽", tone: "warning" };
    case "proxy_disabled":
      return { label: "代理停用", tone: "warning" };
  }
}

function quotaIsExhausted(quota: OAuthQuotaSnapshot | null) {
  if (quota === null) return false;
  const observedExhaustion = quota.accountStatus?.quotaExhaustion;
  const creditsUsable = quota.credits?.unlimited === true
    || quota.credits?.hasCredits === true;
  const workspaceHardStop = quota.access?.spendControlReached === true
    || quota.access?.reachedType === "workspace_owner_credits_depleted"
    || quota.access?.reachedType === "workspace_member_credits_depleted"
    || quota.access?.reachedType === "workspace_owner_usage_limit_reached"
    || quota.access?.reachedType === "workspace_member_usage_limit_reached";
  const rollingLimitReached = quota.rateLimit?.allowed === false
    || quota.rateLimit?.limitReached === true
    || quota.access?.reachedType === "rate_limit_reached";
  return (
    workspaceHardStop
    || rollingLimitReached && !creditsUsable
    || quota.tokenBalance?.remaining === 0
    || observedExhaustion !== null && observedExhaustion !== undefined
  );
}

function formatExpiry(value: number | null) {
  if (value === null) {
    return "未知";
  }
  return new Date(value * 1_000).toLocaleString(undefined, {
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}
