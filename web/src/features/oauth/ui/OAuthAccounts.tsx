import { useQuery } from "@tanstack/react-query";
import { KeyRound } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { OAuthAccount, OAuthProvider } from "../api/oauth-contracts";
import { presentOAuthAccount } from "../model/oauth-account-presentation";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { useOAuthAccountMutations } from "../model/use-oauth-account-mutations";
import { oauthProviderLabel } from "../model/oauth-provider-catalog";
import { oauthQuotaQueryOptions } from "../model/oauth-quota-query";
import { OAuthAccountCard } from "./OAuthAccountCard";
import { OAuthAccountEditor } from "./OAuthAccountEditor";
import { OAuthQuotaPanel } from "./OAuthQuotaPanel";
import { OAuthRefreshFailureNotice } from "./OAuthRefreshFailureNotice";
import type { ProxyConfiguration } from "@/features/proxies";
import { notify } from "@/shared/notifications";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { RequestUsageStats } from "@/shared/ui/RequestUsageStats";
import { VirtualGrid } from "@/shared/ui/VirtualGrid";

interface OAuthAccountsProps {
  provider: OAuthProvider;
  accounts: OAuthAccount[];
  configRevision: number;
  proxyConfiguration: ProxyConfiguration;
  quotaRefreshPending?: boolean;
}

/** Account cards for one provider kind — lives only in the content column. */
export function OAuthAccounts({
  provider,
  accounts,
  configRevision,
  proxyConfiguration,
  quotaRefreshPending = false,
}: OAuthAccountsProps) {
  const mutations = useOAuthAccountMutations();
  const [searchParams, setSearchParams] = useSearchParams();
  const [deleteTarget, setDeleteTarget] = useState<OAuthAccount | null>(null);
  const selectedId = searchParams.get("account");
  const mode = searchParams.get("oauth_action") === "models" ? "models" : "metadata";
  const selected = accounts.find((account) => account.id === selectedId);
  const providerName = oauthProviderLabel(provider);
  const pending = mutations.isPending || quotaRefreshPending;
  const editorError = mode === "models" ? mutations.models.error : mutations.update.error;

  function open(account: OAuthAccount, action: "metadata" | "models") {
    mutations.update.reset();
    mutations.models.reset();
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

  function toggleAccount(account: OAuthAccount, enabled: boolean) {
    mutations.update.mutate(
      {
        id: account.id,
        input: {
          expectedRevision: configRevision,
          expectedConfigVersion: account.configVersion,
          label: account.label,
          requestsPerMinute: account.requestsPerMinute,
          proxySelection: account.proxySelection,
          enabled,
        },
      },
      {
        onSuccess: () => {
          notify.success(enabled ? `已启用「${account.label}」` : `已停用「${account.label}」`);
        },
        onError: (error) => notify.danger(getOAuthErrorMessage(error)),
      },
    );
  }

  return (
    <div className="h-full min-h-0 flex-1" aria-busy={pending}>
      {accounts.length === 0 ? (
        <div
          className="flex h-full min-h-40 flex-col items-center justify-center px-6 py-9 text-center"
          role="status"
          aria-label={`暂无 ${providerName} OAuth 账号`}
        >
          <KeyRound size={20} strokeWidth={1.6} className="text-tertiary" aria-hidden="true" />
          <p className="mt-2.5 text-[13px] font-medium text-primary">
            还没有 {providerName} OAuth 账号
          </p>
        </div>
      ) : (
        <VirtualGrid
          items={accounts}
          getItemKey={(account) => account.id}
          renderItem={(account) => (
            <OAuthAccountItem
              account={account}
              pending={pending}
              quotaRefreshPending={quotaRefreshPending}
              onToggleEnabled={(enabled) => toggleAccount(account, enabled)}
              onViewModels={() => open(account, "models")}
              onEdit={() => open(account, "metadata")}
              onDelete={() => setDeleteTarget(account)}
            />
          )}
          ariaLabel={`${providerName} OAuth 账号列表`}
          collectionKey={provider}
          estimateRowHeight={330}
          minItemWidth={280}
          maxColumns={3}
          gap={12}
          overscanRows={2}
        />
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
            proxyConfiguration={proxyConfiguration}
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
              notify.success(`已保存「${value.label}」`);
            }}
            onSaveModels={async (models) => {
              await mutations.models.mutateAsync({
                id: selected.id,
                input: {
                  expectedRevision: configRevision,
                  expectedConfigVersion: selected.configVersion,
                  models,
                },
              });
              notify.success(`已保存「${selected.label}」的模型选择`);
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
          const target = deleteTarget;
          mutations.remove.mutate(
            {
              id: target.id,
              expectedRevision: configRevision,
              expectedConfigVersion: target.configVersion,
            },
            {
              onSuccess: () => notify.success(`已删除「${target.label}」`),
              onError: (error) => notify.danger(getOAuthErrorMessage(error)),
              onSettled: () => setDeleteTarget(null),
            },
          );
        }}
      />
    </div>
  );
}

function OAuthAccountItem({
  account,
  pending,
  quotaRefreshPending,
  onToggleEnabled,
  onViewModels,
  onEdit,
  onDelete,
}: {
  account: OAuthAccount;
  pending: boolean;
  quotaRefreshPending: boolean;
  onToggleEnabled: (enabled: boolean) => void;
  onViewModels: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const reauthorizationRequired = account.tokenRefreshFailure?.reauthorizationRequired === true;
  const quotaQuery = useQuery({
    ...oauthQuotaQueryOptions(account.id),
    enabled: !reauthorizationRequired,
  });
  const quota = reauthorizationRequired || quotaQuery.isError
    ? null
    : (quotaQuery.data ?? null);
  return (
    <OAuthAccountCard
      presentation={presentOAuthAccount(account, quota)}
      proxyLabel={runtimeProxyLabel(account)}
      pending={pending}
      onToggleEnabled={onToggleEnabled}
      onViewModels={onViewModels}
      onEdit={onEdit}
      onDelete={onDelete}
      lastUpdatedAt={quota?.fetchedAt ?? null}
      details={
        <>
          {account.tokenRefreshFailure ? (
            <OAuthRefreshFailureNotice failure={account.tokenRefreshFailure} />
          ) : null}
          {!reauthorizationRequired ? (
            <>
              <RequestUsageStats label={account.label} usage={account.usage} />
              <OAuthQuotaPanel
                accountId={account.id}
                accountLabel={account.label}
                provider={account.providerKind}
                disabled={pending}
                refreshAllPending={quotaRefreshPending}
                showError={account.tokenRefreshFailure === null}
              />
            </>
          ) : null}
        </>
      }
    />
  );
}

function runtimeProxyLabel(account: OAuthAccount) {
  const proxy = account.runtime.resolvedProxy;
  const selection = account.proxySelection.mode === "global" ? "跟随全局" : "指定";
  const kind = proxy.kind === "direct"
    ? ""
    : `（${proxy.kind === "http" ? "HTTP" : "SOCKS5"}）`;
  return `${selection} · ${proxy.name}${kind}${proxy.enabled ? "" : " · 已停用"}`;
}
