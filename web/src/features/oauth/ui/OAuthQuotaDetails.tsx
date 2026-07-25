import type {
  OAuthQuotaSnapshot,
  OAuthQuotaWindow,
} from "../api/oauth-quota-contracts";
import { cn } from "@/shared/lib/cn";

export function OAuthQuotaDetails({
  quota,
  showResetCredits,
}: {
  quota: OAuthQuotaSnapshot;
  showResetCredits: boolean;
}) {
  const windows = [quota.rateLimit?.primaryWindow, quota.rateLimit?.secondaryWindow]
    .filter((window): window is OAuthQuotaWindow => window !== null && window !== undefined)
    .sort((left, right) => left.limitWindowSeconds - right.limitWindowSeconds);
  const creditExpiry = showResetCredits ? formatCreditExpiries(quota) : null;

  return (
    <div className="mt-2 space-y-2.5">
      {windows.map((window) => (
        <QuotaWindowBar key={`${window.limitWindowSeconds}-${window.resetAt}`} window={window} />
      ))}
      {windows.length === 0 ? (
        <p className="text-[11px] text-tertiary">上游未返回限额窗口</p>
      ) : null}
      {showResetCredits ? (
        <div className="flex items-baseline justify-between gap-2 text-[11px]">
          <span className="text-secondary">重置次数</span>
          <span className="font-medium tabular-nums text-primary">
            {quota.resetCredits?.availableCount ?? "未知"}
          </span>
        </div>
      ) : null}
      {creditExpiry ? (
        <p className="truncate text-[10px] text-tertiary" title={creditExpiry}>
          {creditExpiry}
        </p>
      ) : null}
    </div>
  );
}

function QuotaWindowBar({ window }: { window: OAuthQuotaWindow }) {
  const used = Math.min(100, Math.max(0, window.usedPercent));
  const remaining = Math.max(0, 100 - used);
  const label = windowLabel(window.limitWindowSeconds);
  return (
    <div className="min-w-0">
      <div className="flex items-baseline justify-between gap-2 text-[11px]">
        <span className="min-w-0 truncate text-secondary">{label}</span>
        <span className="shrink-0 tabular-nums text-secondary">
          <span className={cn("font-semibold", remainingTone(remaining))}>
            {remaining.toFixed(0)}%
          </span>
          <span className="ml-1.5 text-tertiary">{formatCompactTime(window.resetAt)}</span>
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

function windowLabel(seconds: number) {
  if (seconds === 18_000 || seconds === 5 * 3_600) return "5 小时限额";
  if (seconds === 604_800 || seconds === 7 * 86_400) return "周限额";
  if (seconds === 30 * 86_400) return "月限额";
  return "限额";
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
    ? `重置次数 ${first} 到期`
    : `重置次数 ${first} 到期，另有 ${expiries.length - 1} 次`;
}

function formatExpiry(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}
