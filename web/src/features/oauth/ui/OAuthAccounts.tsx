import { Edit3, ListChecks, RefreshCw, Trash2 } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { OAuthAccount } from "../api/oauth-contracts";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { useOAuthAccountMutations } from "../model/use-oauth-account-mutations";
import { useOAuthAccounts } from "../model/use-oauth-accounts";
import { OAuthAccountEditor } from "./OAuthAccountEditor";
import { OAuthQuotaPanel } from "./OAuthQuotaPanel";
import { oauthProviderLabel } from "../model/oauth-provider-catalog";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { Surface } from "@/shared/ui/Surface";

export function OAuthAccounts() {
  const accounts = useOAuthAccounts();
  const mutations = useOAuthAccountMutations();
  const [searchParams, setSearchParams] = useSearchParams();
  const [deleteTarget, setDeleteTarget] = useState<OAuthAccount | null>(null);
  const selectedId = searchParams.get("account");
  const mode = searchParams.get("oauth_action") === "models" ? "models" : "metadata";
  const selected = accounts.data?.items.find((account) => account.id === selectedId);

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

  if (accounts.isPending && !accounts.data) {
    return <Surface className="p-6 text-sm text-secondary">正在读取 OAuth 账号…</Surface>;
  }
  if (!accounts.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取 OAuth 账号</p>
        <p className="mt-2 text-sm text-secondary">{getOAuthErrorMessage(accounts.error)}</p>
        <Button className="mt-4" onClick={() => void accounts.refetch()}>
          <RefreshCw size={14} aria-hidden="true" />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = accounts.data;
  const pending = mutations.isPending;
  const editorError = mode === "models" ? mutations.models.error : mutations.update.error;

  return (
    <section aria-busy={pending || accounts.isFetching}>
      <div className="mb-3 flex items-center justify-between gap-3">
        <div>
          <h2 className="font-semibold">已激活账号</h2>
          <p className="mt-1 text-xs text-secondary">Token 与 Provider JSON 只保存在服务器 SQLite 中。</p>
        </div>
        <Button variant="ghost" disabled={accounts.isFetching} onClick={() => void accounts.refetch()}>
          <RefreshCw size={14} aria-hidden="true" />
          刷新账号
        </Button>
      </div>

      {configuration.items.length === 0 ? (
        <Surface className="p-8 text-center text-sm text-secondary">还没有已激活的 OAuth 账号。</Surface>
      ) : (
        <div className="grid gap-3 lg:grid-cols-2">
          {configuration.items.map((account) => (
            <OAuthAccountCard
              key={account.id}
              account={account}
              pending={pending}
              onEdit={() => open(account, "metadata")}
              onModels={() => open(account, "models")}
              onDelete={() => setDeleteTarget(account)}
            />
          ))}
        </div>
      )}

      <SideDrawer
        open={selected !== undefined}
        title={mode === "models" ? "选择 OAuth 模型" : "编辑 OAuth 账号"}
        description="OAuth 账号与 Provider API Key 分开管理。"
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
                  expectedRevision: configuration.configRevision,
                  expectedConfigVersion: selected.configVersion,
                  ...value,
                },
              });
            }}
            onSaveModels={async (models) => {
              await mutations.models.mutateAsync({
                id: selected.id,
                input: {
                  expectedRevision: configuration.configRevision,
                  expectedConfigVersion: selected.configVersion,
                  models,
                },
              });
            }}
          />
        ) : null}
      </SideDrawer>

      <ConfirmDialog
        open={deleteTarget !== null}
        title="删除 OAuth 账号"
        description={deleteTarget ? `确定删除“${deleteTarget.label}”？服务器中的 OAuth Token 将一并删除。` : undefined}
        confirmLabel="删除"
        tone="danger"
        pending={mutations.remove.isPending}
        onClose={() => !mutations.remove.isPending && setDeleteTarget(null)}
        onConfirm={() => {
          if (!deleteTarget) return;
          mutations.remove.mutate(
            {
              id: deleteTarget.id,
              expectedRevision: configuration.configRevision,
              expectedConfigVersion: deleteTarget.configVersion,
            },
            { onSettled: () => setDeleteTarget(null) },
          );
        }}
      />
    </section>
  );
}

function OAuthAccountCard({
  account,
  pending,
  onEdit,
  onModels,
  onDelete,
}: {
  account: OAuthAccount;
  pending: boolean;
  onEdit: () => void;
  onModels: () => void;
  onDelete: () => void;
}) {
  const [renderedAt] = useState(() => Math.floor(Date.now() / 1_000));
  const expired = account.expiresAt !== null && account.expiresAt <= renderedAt;
  return (
    <Surface className="p-5">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="break-words font-semibold">{account.label}</h3>
            <span className="rounded-full bg-surface-muted px-2.5 py-1 text-xs text-secondary">
              {oauthProviderLabel(account.providerKind)}
            </span>
            {!account.enabled || expired ? (
              <span className="rounded-full bg-warning/15 px-2.5 py-1 text-xs text-warning-copy">
                {expired ? "Token 已过期" : "已停用"}
              </span>
            ) : null}
          </div>
          <p className="mt-2 text-sm text-secondary">{account.safeAccountEmail ?? "未提供邮箱"}</p>
          <p className="mt-1 truncate font-mono text-xs text-tertiary" title={account.id}>{account.id}</p>
        </div>
        <Button variant="ghost" disabled={pending} onClick={onDelete} aria-label={`删除 ${account.label}`}>
          <Trash2 size={14} aria-hidden="true" />
        </Button>
      </div>
      <dl className="mt-4 grid grid-cols-2 gap-3 text-sm">
        <Metric label="最大并发" value={String(account.maxConcurrency)} />
        <Metric label="已选模型" value={String(account.selectedModelCount)} />
        <Metric label="Token 版本" value={String(account.tokenVersion)} />
        <Metric label="过期时间" value={formatExpiry(account.expiresAt)} />
      </dl>
      <div className="mt-4 flex justify-end gap-2 border-t border-subtle pt-4">
        <Button variant="ghost" disabled={pending} onClick={onModels}>
          <ListChecks size={14} aria-hidden="true" />
          模型
        </Button>
        <Button disabled={pending} onClick={onEdit}>
          <Edit3 size={14} aria-hidden="true" />
          编辑
        </Button>
      </div>
      {account.providerKind === "codex" ? (
        <OAuthQuotaPanel accountId={account.id} accountLabel={account.label} />
      ) : null}
    </Surface>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return <div><dt className="text-xs text-tertiary">{label}</dt><dd className="mt-1 break-words font-medium">{value}</dd></div>;
}

function formatExpiry(value: number | null) {
  return value === null ? "未知" : new Date(value * 1_000).toLocaleString();
}
