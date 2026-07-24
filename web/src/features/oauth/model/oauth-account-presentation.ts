import type { OAuthAccount } from "../api/oauth-contracts";

/** Neutral chip (plan tier, region, …) or warning (disabled / expired). */
export type OAuthAccountBadgeTone = "neutral" | "warning";

export interface OAuthAccountBadge {
  key: string;
  label: string;
  tone: OAuthAccountBadgeTone;
}

export interface OAuthAccountMetric {
  key: string;
  label: string;
  value: string;
  title?: string;
}

/**
 * Provider-agnostic view model for OAuth account rows and drawers.
 * Codex/Claude/Grok only differ by how fields map into badges/metrics —
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
  nowSeconds: number = Math.floor(Date.now() / 1_000),
): OAuthAccountPresentation {
  const expired = account.expiresAt !== null && account.expiresAt <= nowSeconds;
  const badges: OAuthAccountBadge[] = [];

  // Official provider plan/tier string when present (Codex: chatgpt_plan_type, others later).
  if (account.planType) {
    badges.push({ key: "plan", label: account.planType, tone: "neutral" });
  }
  if (!account.enabled) {
    badges.push({ key: "disabled", label: "已停用", tone: "warning" });
  } else if (expired) {
    badges.push({ key: "expired", label: "已过期", tone: "warning" });
  }

  const metrics: OAuthAccountMetric[] = [
    {
      key: "concurrency",
      label: "并发",
      value: String(account.maxConcurrency),
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
