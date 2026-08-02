import type {
  OAuthQuotaSnapshot,
  OAuthQuotaTokenBalance,
  OAuthQuotaWindow,
} from "../api/oauth-quota-contracts";
import type { OAuthProvider } from "../api/oauth-contracts";
import { cn } from "@/shared/lib/cn";

export function OAuthQuotaDetails({
  quota,
  provider,
  showResetCredits,
}: {
  quota: OAuthQuotaSnapshot;
  provider: OAuthProvider;
  showResetCredits: boolean;
}) {
  const windows = quota.rateLimit?.windows ?? [];
  const creditExpiry = showResetCredits ? formatCreditExpiries(quota) : null;
  const isGrok = provider === "grok";

  return (
    <div className="mt-2 space-y-2.5">
      {isGrok && quota.accountStatus?.userBlockedReason ? (
        <QuotaValue
          label="xAI 用户限制"
          value={quota.accountStatus.userBlockedReason}
        />
      ) : null}
      {isGrok && quota.accountStatus?.teamBlockedReasons.length ? (
        <QuotaValue
          label="xAI 团队策略"
          value={quota.accountStatus.teamBlockedReasons.join(", ")}
        />
      ) : null}
      {quota.tokenBalance ? (
        <TokenBalanceBar balance={quota.tokenBalance} />
      ) : null}
      {windows.map((window) => (
        <QuotaWindowBar
          key={window.id}
          window={window}
        />
      ))}
      {windows.length === 0 && quota.tokenBalance === null ? (
        <p className="text-[11px] text-tertiary">
          {quota.rateLimit?.limitReached
            ? "上游报告额度已用尽"
            : isGrok
              ? grokAvailabilityMessage(quota)
              : "上游未返回限额窗口"}
        </p>
      ) : null}
      {isGrok && typeof quota.billing?.prepaidBalanceMinor === "number" ? (
        <QuotaValue
          label="预付余额"
          value={formatUsdMinor(quota.billing?.prepaidBalanceMinor ?? 0)}
        />
      ) : null}
      {isGrok && hasOnDemandBilling(quota) ? (
        <QuotaValue
          label="按量使用"
          value={`${formatUsdMinor(quota.billing?.onDemandUsedMinor ?? 0)} / ${formatUsdMinor(quota.billing?.onDemandCapMinor ?? 0)}`}
        />
      ) : null}
      {creditExpiry ? (
        <p className="truncate text-[10px] text-tertiary" title={creditExpiry}>
          {creditExpiry}
        </p>
      ) : null}
    </div>
  );
}

function grokAvailabilityMessage(quota: OAuthQuotaSnapshot) {
  const exhausted = quota.accountStatus?.quotaExhaustion;
  if (exhausted) {
    const amount = exhausted.used !== null && exhausted.limit !== null
      ? `：${exhausted.used.toLocaleString()} / ${exhausted.limit.toLocaleString()}`
      : "";
    return `最近真实请求已确认额度耗尽${amount} · ${formatCompactTime(exhausted.observedAt)}`;
  }
  return quota.subscriptionTier?.trim().toLowerCase() === "free"
    ? "xAI 未返回可计量的 Free 余额"
    : "xAI 未返回订阅使用率";
}

function TokenBalanceBar({
  balance,
}: {
  balance: OAuthQuotaTokenBalance;
}) {
  const remainingPercent = balance.limit === 0
    ? 0
    : Math.min(100, Math.max(0, balance.remaining / balance.limit * 100));
  return (
    <div className="min-w-0">
      <div className="flex items-baseline justify-between gap-2 text-[11px]">
        <span className="min-w-0 truncate text-secondary">Token 余额 · 上游真实观测</span>
        <span className={cn("shrink-0 font-semibold tabular-nums", remainingTone(remainingPercent))}>
          {balance.remaining.toLocaleString()} / {balance.limit.toLocaleString()}
        </span>
      </div>
      <div
        className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-surface-muted"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={balance.limit}
        aria-valuenow={balance.remaining}
        aria-label={`Token 剩余 ${balance.remaining.toLocaleString()} / ${balance.limit.toLocaleString()}`}
      >
        <div
          className={cn(
            "h-full rounded-full transition-[width] duration-200",
            remainingBar(remainingPercent),
          )}
          style={{ width: `${remainingPercent}%` }}
        />
      </div>
    </div>
  );
}

function QuotaValue({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-2 text-[11px]">
      <span className="text-secondary">{label}</span>
      <span className="min-w-0 truncate font-medium tabular-nums text-primary" title={value}>
        {value}
      </span>
    </div>
  );
}

function hasOnDemandBilling(quota: OAuthQuotaSnapshot) {
  const used = quota.billing?.onDemandUsedMinor;
  const cap = quota.billing?.onDemandCapMinor;
  return (used !== null && used !== undefined && used !== 0)
    || (cap !== null && cap !== undefined && cap !== 0);
}

function formatUsdMinor(value: number) {
  return `$${(Math.abs(value) / 100).toFixed(2)}`;
}

function QuotaWindowBar({ window }: { window: OAuthQuotaWindow }) {
  const used = Math.min(100, Math.max(0, window.usedPercent));
  const remaining = Math.max(0, 100 - used);
  const label = windowLabel(window);
  const reset = window.resetAt === null ? null : formatCompactTime(window.resetAt);
  return (
    <div className="min-w-0">
      <div className="flex items-baseline justify-between gap-2 text-[11px]">
        <span className="min-w-0 truncate text-secondary">{label}</span>
        <span className="shrink-0 tabular-nums text-secondary">
          <span className={cn("font-semibold", remainingTone(remaining))}>
            {remaining.toFixed(0)}%
          </span>
          {reset ? <span className="ml-1.5 text-tertiary">{reset}</span> : null}
        </span>
      </div>
      <div
        className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-surface-muted"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={remaining}
        aria-label={`${label} 剩余 ${remaining.toFixed(1)}%`}
      >
        <div
          className={cn(
            "h-full rounded-full transition-[width] duration-200",
            remainingBar(remaining),
          )}
          style={{ width: `${remaining}%` }}
          title={`剩余 ${remaining.toFixed(1)}% · 已用 ${used.toFixed(1)}%`}
        />
      </div>
    </div>
  );
}

function remainingTone(remaining: number) {
  if (remaining <= 10) return "text-danger";
  if (remaining <= 30) return "text-warning";
  return "text-primary";
}

function remainingBar(remaining: number) {
  if (remaining <= 10) return "bg-danger";
  if (remaining <= 30) return "bg-warning";
  return "bg-success";
}

function windowLabel(window: OAuthQuotaWindow) {
  if (window.id === "five_hour") return "5 小时限额";
  if (window.id === "seven_day") return "7 天限额";
  if (window.id === "seven_day_sonnet") return "Sonnet 7 天限额";
  if (window.id === "seven_day_overage_included") return "Fable 7 天限额";
  const seconds = window.limitWindowSeconds;
  if (seconds === 18_000 || seconds === 5 * 3_600) return "5 小时限额";
  if (seconds === 604_800 || seconds === 7 * 86_400) return "周限额";
  if (seconds === 30 * 86_400) return "月限额";
  return window.kind === "credits" ? "Credits 限额" : "限额";
}

function formatCompactTime(value: number) {
  return new Date(value * 1_000).toLocaleString(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

function formatCreditExpiries(quota: OAuthQuotaSnapshot): string | null {
  const expiries = quota.resetCredits?.expiresAt ?? [];
  if (expiries.length === 0) return null;
  const first = formatExpiry(expiries[0]);
  return expiries.length === 1
    ? `最早 ${first} 到期`
    : `最早 ${first} 到期 · 共 ${expiries.length} 次`;
}

function formatExpiry(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}
