import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { OAuthAccount, OAuthProvider } from "../api/oauth-contracts";
import { presentOAuthAccount } from "../model/oauth-account-presentation";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { useOAuthAccountMutations } from "../model/use-oauth-account-mutations";
import { oauthProviderLabel } from "../model/oauth-provider-catalog";
import { OAuthAccountCard } from "./OAuthAccountCard";
import { OAuthAccountEditor } from "./OAuthAccountEditor";
import { OAuthQuotaPanel } from "./OAuthQuotaPanel";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { Surface } from "@/shared/ui/Surface";
import { RequestUsageStats } from "@/shared/ui/RequestUsageStats";

interface OAuthAccountsProps {
  provider: OAuthProvider;
  accounts: OAuthAccount[];
  configRevision: number;
}

/** Account cards for one provider kind — lives only in the content column. */
export function OAuthAccounts({
  provider,
  accounts,
  configRevision,
}: OAuthAccountsProps) {
  const mutations = useOAuthAccountMutations();
  const [searchParams, setSearchParams] = useSearchParams();
  const [deleteTarget, setDeleteTarget] = useState<OAuthAccount | null>(null);
  const selectedId = searchParams.get("account");
  const mode = searchParams.get("oauth_action") === "models" ? "models" : "metadata";
  const selected = accounts.find((account) => account.id === selectedId);
  const providerName = oauthProviderLabel(provider);
  const pending = mutations.isPending;
  const editorError = mutations.update.error;

  function open(account: OAuthAccount, action: "metadata" | "models") {
    mutations.update.reset();
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.set("account", account.id);
        next.set("oauth_action", action);
        return next;
      },
      { replace: true },
    );
  }

  function close() {
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.delete("account");
        next.delete("oauth_action");
        return next;
      },
      { replace: true },
    );
  }

  return (
    <div aria-busy={pending}>
      {accounts.length === 0 ? (
        <Surface className="flex min-h-48 flex-col items-center justify-center px-4 py-10 text-center">
          <p className="text-[13px] font-medium">还没有 {providerName} OAuth 账号</p>
          <p className="mt-1 max-w-sm text-[12px] text-secondary">
            点击「OAuth认证」生成一次性授权链接，完成后粘贴回调 URL 激活服务器账号。
          </p>
        </Surface>
      ) : (
        <div className="space-y-2">
          {accounts.map((account) => (
            <OAuthAccountCard
              key={account.id}
              presentation={presentOAuthAccount(account)}
              pending={pending}
              onToggleEnabled={(enabled) => {
                mutations.update.mutate({
                  id: account.id,
                  input: {
                    expectedRevision: configRevision,
                    expectedConfigVersion: account.configVersion,
                    label: account.label,
                    maxConcurrency: account.maxConcurrency,
                    enabled,
                  },
                });
              }}
              onViewModels={() => open(account, "models")}
              onEdit={() => open(account, "metadata")}
              onDelete={() => setDeleteTarget(account)}
              details={
                <>
                  <div className="mt-2.5 border-t border-subtle pt-2.5">
                    <RequestUsageStats label={account.label} usage={account.usage} />
                  </div>
                  {account.providerKind === "codex" ? (
                    <OAuthQuotaPanel accountId={account.id} accountLabel={account.label} />
                  ) : null}
                </>
              }
            />
          ))}
        </div>
      )}

      {mutations.remove.error ? (
        <p className="pt-2 text-sm text-danger" role="alert">
          {getOAuthErrorMessage(mutations.remove.error)}
        </p>
      ) : null}

      <SideDrawer
        open={selected !== undefined}
        title={mode === "models" ? "可用模型" : "编辑 OAuth 账号"}
        description={mode === "models" ? undefined : "OAuth 账号与 Provider API Key 分开管理。"}
        onClose={close}
      >
        {selected ? (
          <OAuthAccountEditor
            key={`${selected.id}:${selected.configVersion}:${mode}`}
            account={selected}
            mode={mode}
            pending={pending}
            error={editorError}
            onClose={close}
            onSaveMetadata={async (value) => {
              await mutations.update.mutateAsync({
                id: selected.id,
                input: {
                  expectedRevision: configRevision,
                  expectedConfigVersion: selected.configVersion,
                  ...value,
                },
              });
            }}
          />
        ) : null}
      </SideDrawer>

      <ConfirmDialog
        open={deleteTarget !== null}
        title="删除 OAuth 账号"
        description={
          deleteTarget ? `确定删除“${deleteTarget.label}”？服务器中的 OAuth Token 将一并删除。` : undefined
        }
        confirmLabel="删除"
        tone="danger"
        pending={mutations.remove.isPending}
        onClose={() => !mutations.remove.isPending && setDeleteTarget(null)}
        onConfirm={() => {
          if (!deleteTarget) return;
          mutations.remove.mutate(
            {
              id: deleteTarget.id,
              expectedRevision: configRevision,
              expectedConfigVersion: deleteTarget.configVersion,
            },
            { onSettled: () => setDeleteTarget(null) },
          );
        }}
      />
    </div>
  );
}
