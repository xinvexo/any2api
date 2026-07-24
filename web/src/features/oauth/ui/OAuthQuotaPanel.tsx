import {
  useIsMutating,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { RefreshCw, RotateCcw } from "lucide-react";
import { useRef, useState } from "react";

import type {
  OAuthQuotaSnapshot,
  OAuthQuotaWindow,
} from "../api/oauth-quota-contracts";
import { resetOAuthAccountQuota } from "../api/oauth-api";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { oauthQueryKeys } from "../model/oauth-query-keys";
import {
  oauthQuotaQueryOptions,
  refreshOAuthAccountQuota,
} from "../model/oauth-quota-query";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";

export function OAuthQuotaPanel({
  accountId,
  accountLabel,
  disabled = false,
}: {
  accountId: string;
  accountLabel: string;
  disabled?: boolean;
}) {
  const queryClient = useQueryClient();
  const quotaOptions = oauthQuotaQueryOptions(accountId);
  const quotaQuery = useQuery({ ...quotaOptions, enabled: false });
  const resetRequested = useRef(false);
  const [resetRefreshErrorAt, setResetRefreshErrorAt] = useState<number | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const resetMutationKey = oauthQueryKeys.quotaReset(accountId);
  const resetMutation = useMutation({
    mutationKey: resetMutationKey,
    retry: false,
    mutationFn: async () => {
      const result = await resetOAuthAccountQuota(accountId);
      await queryClient.resetQueries({ queryKey: quotaOptions.queryKey, exact: true });
      try {
        await refreshOAuthAccountQuota(queryClient, accountId);
        return { ...result, quotaRefreshed: true, quotaErrorUpdatedAt: null };
      } catch {
        return {
          ...result,
          quotaRefreshed: false,
          quotaErrorUpdatedAt:
            queryClient.getQueryState(quotaOptions.queryKey)?.errorUpdatedAt
            ?? Date.now(),
        };
      }
    },
    onSuccess: (result) => {
      setSuccess(`已重置 ${result.windowsReset} 个额度窗口。`);
      setResetRefreshErrorAt(result.quotaErrorUpdatedAt);
    },
    onError: () => setResetRefreshErrorAt(null),
  });
  const resetPending =
    useIsMutating({ mutationKey: resetMutationKey, exact: true }) > 0;
  const quota = quotaQuery.data ?? null;
  const pending = resetPending ? "reset" : quotaQuery.isFetching ? "query" : null;
  const resetRefreshError =
    resetRefreshErrorAt !== null
    && quotaQuery.isError
    && quotaQuery.errorUpdatedAt === resetRefreshErrorAt
      ? "额度已重置，但最新额度读取失败。"
      : null;
  const visibleError =
    (resetMutation.isError ? getOAuthErrorMessage(resetMutation.error) : null)
    ?? resetRefreshError
    ?? (!quotaQuery.isFetching && quotaQuery.isError
        ? getOAuthErrorMessage(quotaQuery.error)
        : null);
  const availableCount = quota?.resetCredits?.availableCount ?? 0;

  async function refreshQuota() {
    setResetRefreshErrorAt(null);
    setSuccess(null);
    resetMutation.reset();
    try {
      await refreshOAuthAccountQuota(queryClient, accountId);
    } catch {
      // The query cache owns the account-scoped error rendered below.
    }
  }

  function reset() {
    if (disabled || resetRequested.current || resetPending) {
      return;
    }
    setConfirmOpen(false);
    setResetRefreshErrorAt(null);
    setSuccess(null);
    resetMutation.reset();
    resetRequested.current = true;
    resetMutation.mutate(undefined, {
      onSettled: () => {
        resetRequested.current = false;
      },
    });
  }

  return (
    <section aria-label="Codex 额度" className="mt-2 border-t border-subtle/50 pt-2">
      <div className="flex items-center justify-between gap-2">
        <p className="text-[11px] font-medium text-secondary">Codex 额度</p>
        <div className="flex items-center gap-0.5">
          <Button
            variant="ghost"
            size="sm"
            className="h-6 min-h-6 px-1.5 text-[11px]"
            disabled={disabled || pending !== null}
            onClick={() => void refreshQuota()}
          >
            <RefreshCw
              size={12}
              className={pending === "query" ? "animate-spin" : undefined}
              aria-hidden="true"
            />
            刷新额度
          </Button>
          <Button
            variant="danger"
            size="sm"
            className="h-6 min-h-6 px-1.5 text-[11px]"
            disabled={disabled || pending !== null || availableCount === 0}
            title={
              disabled
                ? "刷新全部额度进行中"
                : quota === null
                ? "请先刷新额度"
                : availableCount === 0
                  ? "没有可用的重置次数"
                  : undefined
            }
            onClick={() => setConfirmOpen(true)}
          >
            <RotateCcw
              size={12}
              className={pending === "reset" ? "animate-spin" : undefined}
              aria-hidden="true"
            />
            重置额度
          </Button>
        </div>
      </div>

      {quota ? (
        <QuotaDetails quota={quota} />
      ) : (
        <p className="mt-1.5 text-[11px] text-tertiary">额度尚未刷新</p>
      )}
      {visibleError ? (
        <p className="mt-1.5 text-[11px] text-danger" role="alert">
          {visibleError}
        </p>
      ) : null}
      {success ? (
        <p className="mt-1.5 text-[11px] text-success" role="status">
          {success}
        </p>
      ) : null}

      <ConfirmDialog
        open={confirmOpen}
        title="确认重置 Codex 额度"
        description={`将为“${accountLabel}”消耗 1 次重置次数并立即恢复当前额度窗口。当前剩余 ${availableCount} 次。`}
        confirmLabel="重置额度"
        tone="danger"
        pending={pending === "reset"}
        onClose={() => setConfirmOpen(false)}
        onConfirm={reset}
      />
    </section>
  );
}

function QuotaDetails({ quota }: { quota: OAuthQuotaSnapshot }) {
  // Only show windows the upstream actually returned. Codex may send one of
  // {5h}, {week}, {month}, or pairs like {5h + week} — never invent empty rows.
  const windows = [quota.rateLimit?.primaryWindow, quota.rateLimit?.secondaryWindow]
    .filter((window): window is OAuthQuotaWindow => window !== null && window !== undefined)
    .sort((left, right) => left.limitWindowSeconds - right.limitWindowSeconds);
  const creditExpiry = formatCreditExpiries(quota);

  return (
    <div className="mt-2 space-y-2.5">
      {windows.map((window) => (
        <QuotaWindowBar key={`${window.limitWindowSeconds}-${window.resetAt}`} window={window} />
      ))}
      {windows.length === 0 ? (
        <p className="text-[11px] text-tertiary">上游未返回限额窗口</p>
      ) : null}
      <div className="flex items-baseline justify-between gap-2 text-[11px]">
        <span className="text-secondary">重置次数</span>
        <span className="font-medium tabular-nums text-primary">
          {quota.resetCredits?.availableCount ?? "未知"}
        </span>
      </div>
      {/* Credit expiry ≠ window reset time; only show when upstream gave real dates. */}
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

/** Codex only surfaces 5h / weekly / monthly windows — no "N 天限额" wording. */
function windowLabel(seconds: number) {
  if (seconds === 18_000 || seconds === 5 * 3_600) return "5 小时限额";
  if (seconds === 604_800 || seconds === 7 * 86_400) return "周限额";
  if (seconds === 30 * 86_400) return "月限额";
  // Unknown duration: keep neutral, never invent "天限".
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
