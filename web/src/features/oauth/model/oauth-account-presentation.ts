import type { OAuthAccount } from "../api/oauth-contracts";
import type { OAuthQuotaSnapshot } from "../api/oauth-quota-contracts";

/** Neutral chip (plan tier, region, …) or warning (disabled / expired). */
type OAuthAccountBadgeTone = "neutral" | "warning";

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
  const expired = account.expiresAt !== null && account.expiresAt <= nowSeconds;
  const badges: OAuthAccountBadge[] = [];

  const planType = quota?.subscriptionTier ?? account.planType;
  if (planType) {
    badges.push({ key: "plan", label: planType, tone: "neutral" });
  }
  if (!account.enabled) {
    badges.push({ key: "disabled", label: "已停用", tone: "warning" });
  } else if (expired) {
    badges.push({ key: "expired", label: "已过期", tone: "warning" });
  }
  if (quotaIsExhausted(quota)) {
    badges.push({ key: "quota-exhausted", label: "额度耗尽", tone: "warning" });
  }
  if (account.botFlagged === true) {
    badges.push({ key: "bot-flagged", label: "机器人账号", tone: "warning" });
  }
  if (quota?.accountStatus?.userBlockedReason) {
    badges.push({ key: "upstream-restricted", label: "xAI 受限", tone: "warning" });
  }
  if (account.tokenRefreshFailure) {
    badges.push({
      key: "token-refresh-failed",
      label: account.tokenRefreshFailure.reauthorizationRequired
        ? "需重新授权"
        : "Token 刷新异常",
      tone: "warning",
    });
  }

  const metrics: OAuthAccountMetric[] = [
    {
      key: "rpm",
      label: "RPM",
      value: account.requestsPerMinute === null ? "无限制" : String(account.requestsPerMinute),
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

function quotaIsExhausted(quota: OAuthQuotaSnapshot | null) {
  if (quota === null) return false;
  const observedExhaustion = quota.accountStatus?.quotaExhaustion;
  return (
    quota.rateLimit?.allowed === false
    || quota.rateLimit?.limitReached === true
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
