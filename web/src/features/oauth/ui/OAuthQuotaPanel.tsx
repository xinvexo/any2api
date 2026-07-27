import {
  useIsMutating,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { RefreshCw, RotateCcw } from "lucide-react";
import { useRef, useState } from "react";

import type { OAuthProvider } from "../api/oauth-contracts";
import { resetOAuthAccountQuota } from "../api/oauth-api";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { oauthQueryKeys } from "../model/oauth-query-keys";
import { oauthProviderLabel } from "../model/oauth-provider-catalog";
import {
  oauthQuotaQueryOptions,
  refreshOAuthAccountQuota,
} from "../model/oauth-quota-query";
import { OAuthQuotaDetails } from "./OAuthQuotaDetails";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { notify } from "@/shared/notifications";

export function OAuthQuotaPanel({
  accountId,
  accountLabel,
  provider,
  disabled = false,
}: {
  accountId: string;
  accountLabel: string;
  provider: OAuthProvider;
  disabled?: boolean;
}) {
  const queryClient = useQueryClient();
  const quotaOptions = oauthQuotaQueryOptions(accountId);
  const quotaQuery = useQuery({ ...quotaOptions, enabled: false });
  const resetRequested = useRef(false);
  const [resetRefreshErrorAt, setResetRefreshErrorAt] = useState<number | null>(null);
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
      notify.success(`已重置 ${result.windowsReset} 个额度窗口。`);
      setResetRefreshErrorAt(result.quotaErrorUpdatedAt);
    },
    onError: () => setResetRefreshErrorAt(null),
  });
  const resetPending =
    useIsMutating({ mutationKey: resetMutationKey, exact: true }) > 0;
  const quota = quotaQuery.isError && !quotaQuery.isFetching ? null : (quotaQuery.data ?? null);
  const providerName = oauthProviderLabel(provider);
  const canReset = provider === "codex";
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
    resetMutation.reset();
    resetRequested.current = true;
    resetMutation.mutate(undefined, {
      onSettled: () => {
        resetRequested.current = false;
      },
    });
  }

  return (
    <section
      aria-label={`${providerName} 额度`}
      className="mt-2 border-t border-subtle/50 pt-2"
    >
      <div className="flex items-center justify-between gap-2">
        <p className="text-[11px] font-medium text-secondary">{providerName} 额度</p>
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
          {canReset ? (
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
          ) : null}
        </div>
      </div>

      {quota ? (
        <OAuthQuotaDetails
          quota={quota}
          provider={provider}
          showResetCredits={canReset}
        />
      ) : (
        <p className="mt-1.5 text-[11px] text-tertiary">额度尚未刷新</p>
      )}
      {visibleError ? (
        <p className="mt-1.5 text-[11px] text-danger" role="alert">
          {visibleError}
        </p>
      ) : null}

      {canReset ? (
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
      ) : null}
    </section>
  );
}
