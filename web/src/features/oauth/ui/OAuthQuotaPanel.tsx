import { Gauge, RefreshCw, RotateCcw } from "lucide-react";
import { useState } from "react";

import type {
  OAuthQuotaSnapshot,
  OAuthQuotaWindow,
} from "../api/oauth-quota-contracts";
import {
  getOAuthAccountQuota,
  resetOAuthAccountQuota,
} from "../api/oauth-api";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";

export function OAuthQuotaPanel({ accountId, accountLabel }: {
  accountId: string;
  accountLabel: string;
}) {
  const [quota, setQuota] = useState<OAuthQuotaSnapshot | null>(null);
  const [pending, setPending] = useState<"query" | "reset" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const availableCount = quota?.resetCredits?.availableCount ?? 0;

  async function query() {
    setPending("query");
    setError(null);
    setSuccess(null);
    try {
      setQuota(await getOAuthAccountQuota(accountId));
    } catch (cause) {
      setError(getOAuthErrorMessage(cause));
    } finally {
      setPending(null);
    }
  }

  async function reset() {
    setConfirmOpen(false);
    setPending("reset");
    setError(null);
    setSuccess(null);
    let windowsReset: number;
    try {
      windowsReset = (await resetOAuthAccountQuota(accountId)).windowsReset;
    } catch (cause) {
      setError(getOAuthErrorMessage(cause));
      setPending(null);
      return;
    }
    setSuccess(`已重置 ${windowsReset} 个额度窗口。`);
    try {
      setQuota(await getOAuthAccountQuota(accountId));
    } catch {
      setQuota(null);
      setError("额度已重置，但最新额度读取失败。");
    } finally {
      setPending(null);
    }
  }

  return (
    <section aria-label="Codex 额度" className="mt-4 border-t border-subtle pt-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-sm font-medium">
          <Gauge size={15} aria-hidden="true" />
          Codex 额度
        </div>
        <div className="flex items-center gap-1.5">
          <Button
            variant="ghost"
            disabled={pending !== null}
            onClick={() => void query()}
          >
            <RefreshCw
              size={14}
              className={pending === "query" ? "animate-spin" : undefined}
              aria-hidden="true"
            />
            刷新额度
          </Button>
          <Button
            variant="danger"
            disabled={pending !== null || availableCount === 0}
            title={quota === null ? "请先刷新额度" : availableCount === 0 ? "没有可用的重置次数" : undefined}
            onClick={() => setConfirmOpen(true)}
          >
            <RotateCcw
              size={14}
              className={pending === "reset" ? "animate-spin" : undefined}
              aria-hidden="true"
            />
            重置额度
          </Button>
        </div>
      </div>

      {quota ? <QuotaDetails quota={quota} /> : (
        <p className="mt-3 text-xs text-tertiary">额度尚未刷新</p>
      )}
      {error ? <p className="mt-3 text-xs text-danger" role="alert">{error}</p> : null}
      {success ? <p className="mt-3 text-xs text-success" role="status">{success}</p> : null}

      <ConfirmDialog
        open={confirmOpen}
        title="确认重置 Codex 额度"
        description={`将为“${accountLabel}”消耗 1 次重置次数并立即恢复当前额度窗口。当前剩余 ${availableCount} 次。`}
        confirmLabel="重置额度"
        tone="danger"
        pending={pending === "reset"}
        onClose={() => setConfirmOpen(false)}
        onConfirm={() => void reset()}
      />
    </section>
  );
}

function QuotaDetails({ quota }: { quota: OAuthQuotaSnapshot }) {
  const windows = [
    ["主窗口", quota.rateLimit?.primaryWindow],
    ["次窗口", quota.rateLimit?.secondaryWindow],
  ] as const;
  return (
    <div className="mt-3 grid gap-3 sm:grid-cols-3">
      {windows.map(([fallbackLabel, window]) => (
        <QuotaWindowMetric key={fallbackLabel} fallbackLabel={fallbackLabel} window={window ?? null} />
      ))}
      <div className="min-w-0">
        <p className="text-xs text-tertiary">重置次数</p>
        <p className="mt-1 text-sm font-semibold tabular-nums">
          {quota.resetCredits?.availableCount ?? "未知"}
        </p>
        <p className="mt-1 truncate text-xs text-tertiary" title={formatCreditExpiries(quota)}>
          {formatCreditExpiries(quota)}
        </p>
      </div>
    </div>
  );
}

function QuotaWindowMetric({ fallbackLabel, window }: {
  fallbackLabel: string;
  window: OAuthQuotaWindow | null;
}) {
  if (!window) {
    return <div><p className="text-xs text-tertiary">{fallbackLabel}</p><p className="mt-1 text-sm">无数据</p></div>;
  }
  const percent = Math.min(100, Math.max(0, window.usedPercent));
  return (
    <div className="min-w-0">
      <div className="flex items-baseline justify-between gap-2">
        <p className="text-xs text-tertiary">{windowLabel(window.limitWindowSeconds, fallbackLabel)}</p>
        <p className="text-xs font-medium tabular-nums">{window.usedPercent.toFixed(1)}%</p>
      </div>
      <div
        className="mt-2 h-1.5 overflow-hidden rounded-full bg-surface-muted"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
      >
        <div className="h-full bg-accent transition-[width]" style={{ width: `${percent}%` }} />
      </div>
      <p className="mt-1 truncate text-xs text-tertiary" title={formatTimestamp(window.resetAt)}>
        {formatTimestamp(window.resetAt)} 重置
      </p>
    </div>
  );
}

function windowLabel(seconds: number, fallback: string) {
  if (seconds === 18_000) return "5 小时窗口";
  if (seconds === 604_800) return "7 天窗口";
  if (seconds > 0 && seconds % 86_400 === 0) return `${seconds / 86_400} 天窗口`;
  if (seconds > 0 && seconds % 3_600 === 0) return `${seconds / 3_600} 小时窗口`;
  return fallback;
}

function formatTimestamp(value: number) {
  return new Date(value * 1_000).toLocaleString();
}

function formatCreditExpiries(quota: OAuthQuotaSnapshot) {
  const expiries = quota.resetCredits?.expiresAt ?? [];
  if (expiries.length === 0) return "到期时间未知";
  const first = formatExpiry(expiries[0]);
  return expiries.length === 1 ? `${first} 到期` : `${first} 到期，另有 ${expiries.length - 1} 次`;
}

function formatExpiry(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}
