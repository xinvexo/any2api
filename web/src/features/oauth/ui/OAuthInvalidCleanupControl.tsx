import { RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import type { OAuthAccount, OAuthProvider } from "../api/oauth-contracts";
import type { InvalidOAuthAccountInspection } from "../model/oauth-invalid-account-cleanup";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { oauthProviderLabel } from "../model/oauth-provider-catalog";
import { useOAuthInvalidAccountCleanup } from "../model/use-oauth-invalid-account-cleanup";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";

interface OAuthInvalidCleanupControlProps {
  provider: OAuthProvider;
  accounts: readonly OAuthAccount[];
  disabled?: boolean;
  onBusyChange: (busy: boolean) => void;
}

export function OAuthInvalidCleanupControl({
  provider,
  accounts,
  disabled = false,
  onBusyChange,
}: OAuthInvalidCleanupControlProps) {
  const cleanup = useOAuthInvalidAccountCleanup();
  const [inspection, setInspection] =
    useState<InvalidOAuthAccountInspection | null>(null);
  const providerName = oauthProviderLabel(provider);
  const busy = cleanup.pending || inspection !== null;

  useEffect(() => {
    onBusyChange(busy);
    return () => onBusyChange(false);
  }, [busy, onBusyChange]);

  async function inspect() {
    try {
      const result = await cleanup.inspect(accounts.map((account) => account.id));
      if (!result) return;
      if (result.candidates.length === 0) {
        const message = formatNoInvalidAccounts(providerName, result.inconclusive);
        if (result.inconclusive > 0) {
          notify.warning(message);
        } else {
          notify.info(message);
        }
        return;
      }
      setInspection(result);
    } catch (error) {
      notify.danger(`无法检查无效 OAuth 账号：${getOAuthErrorMessage(error)}`);
    }
  }

  async function remove() {
    if (!inspection) return;
    try {
      const result = await cleanup.remove(inspection.candidates);
      if (!result) return;
      const message = formatDeletionResult(providerName, result);
      if (result.failed > 0) {
        notify.danger(message);
      } else if (result.skipped > 0) {
        notify.warning(message);
      } else {
        notify.success(message);
      }
      setInspection(null);
    } catch (error) {
      notify.danger(`无法删除无效 OAuth 账号：${getOAuthErrorMessage(error)}`);
    }
  }

  const inspecting = cleanup.phase === "inspecting";
  return (
    <>
      <Button
        variant="danger"
        disabled={disabled || busy || accounts.length === 0}
        onClick={() => void inspect()}
      >
        {inspecting ? (
          <RefreshCw size={14} className="animate-spin" aria-hidden="true" />
        ) : (
          <Trash2 size={14} aria-hidden="true" />
        )}
        {inspecting ? "正在检查" : "删除失效账号"}
      </Button>

      <ConfirmDialog
        open={inspection !== null}
        title="删除失效账号"
        description={inspection ? cleanupDescription(providerName, inspection) : undefined}
        confirmLabel={
          inspection ? `删除 ${inspection.candidates.length} 个账号` : "删除"
        }
        tone="danger"
        pending={cleanup.phase === "deleting"}
        onClose={() => !cleanup.pending && setInspection(null)}
        onConfirm={() => void remove()}
      />
    </>
  );
}

function cleanupDescription(
  providerName: string,
  inspection: InvalidOAuthAccountInspection,
) {
  const labels = inspection.candidates.map((candidate) => candidate.label);
  const preview = labels.slice(0, 6).join("、");
  const remainder = labels.length > 6 ? ` 等 ${labels.length} 个账号` : "";
  return (
    <div className="space-y-2">
      <p>
        已确认 {labels.length} 个 {providerName} 账号在刷新 Token
        后仍被上游拒绝。确认后将永久删除账号配置和服务器中的 OAuth Token。
      </p>
      <p className="break-words text-primary">目标：{preview}{remainder}</p>
      {inspection.inconclusive > 0 ? (
        <p>另有 {inspection.inconclusive} 个账号无法确认，均会保留。</p>
      ) : null}
    </div>
  );
}

function formatNoInvalidAccounts(providerName: string, inconclusive: number) {
  const suffix = inconclusive > 0
    ? `；${inconclusive} 个账号因其他错误无法确认，均已保留`
    : "";
  return `未发现明确认证失效的 ${providerName} OAuth 账号${suffix}。`;
}

function formatDeletionResult(
  providerName: string,
  result: { deleted: number; skipped: number; failed: number },
) {
  if (result.skipped === 0 && result.failed === 0) {
    return `已删除 ${result.deleted} 个无效的 ${providerName} OAuth 账号。`;
  }
  return `无效 ${providerName} OAuth 清理完成：删除 ${result.deleted} 个，跳过 ${result.skipped} 个，失败 ${result.failed} 个。`;
}
