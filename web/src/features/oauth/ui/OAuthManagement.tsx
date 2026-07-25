import { LogIn, RefreshCw, Upload } from "lucide-react";
import { useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { OAuthProvider } from "../api/oauth-contracts";
import { getOAuthErrorMessage } from "../model/oauth-error";
import {
  isOAuthProvider,
  oauthProviderLabel,
  OAUTH_PROVIDER_OPTIONS,
} from "../model/oauth-provider-catalog";
import { useOAuthAccounts } from "../model/use-oauth-accounts";
import { useOAuthLogin } from "../model/use-oauth-login";
import { useOAuthQuotaRefreshAll } from "../model/use-oauth-quota-refresh-all";
import { OAuthAccounts } from "./OAuthAccounts";
import { OAuthLoginDrawer } from "./OAuthLogin";
import { OAuthImportDrawer } from "./OAuthImport";
import { OAuthProviderNav } from "./OAuthProviderNav";
import { Button } from "@/shared/ui/Button";
import { KindSplitLayout } from "@/shared/ui/KindSplitLayout";
import { Surface } from "@/shared/ui/Surface";
import { notify } from "@/shared/notifications";

/** Shares KindSplitLayout with 上游提供 so route switches keep chrome geometry. */
export function OAuthManagement() {
  const accounts = useOAuthAccounts();
  const login = useOAuthLogin();
  const quotaRefresh = useOAuthQuotaRefreshAll();
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedProvider = resolveSelectedProvider(searchParams.get("kind"));
  const [loginOpen, setLoginOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);

  const counts = useMemo(() => {
    const next = Object.fromEntries(
      OAUTH_PROVIDER_OPTIONS.map((option) => [option.provider, 0]),
    ) as Record<OAuthProvider, number>;
    for (const account of accounts.data?.items ?? []) {
      next[account.providerKind] = (next[account.providerKind] ?? 0) + 1;
    }
    return next;
  }, [accounts.data?.items]);

  const kindAccounts = useMemo(
    () =>
      (accounts.data?.items ?? []).filter(
        (account) => account.providerKind === selectedProvider,
      ),
    [accounts.data?.items, selectedProvider],
  );

  function selectProvider(next: OAuthProvider) {
    if (next === selectedProvider) {
      return;
    }
    setLoginOpen(false);
    setImportOpen(false);
    login.reset();
    setSearchParams(
      (current) => {
        const params = new URLSearchParams(current);
        params.set("kind", next);
        params.delete("account");
        params.delete("oauth_action");
        return params;
      },
      { replace: true },
    );
  }

  function openLogin() {
    setImportOpen(false);
    setLoginOpen(true);
    void login.start(selectedProvider).catch(() => {
      // Drawer keeps the safe user-facing error for retry.
    });
  }

  function openImport() {
    setLoginOpen(false);
    login.reset();
    setImportOpen(true);
  }

  function closeLogin() {
    setLoginOpen(false);
    login.reset();
  }

  async function refreshAllQuotas() {
    const provider = selectedProvider;
    if (provider === "claude" || kindAccounts.length === 0) {
      return;
    }
    const result = await quotaRefresh.refresh(kindAccounts.map((account) => account.id));
    if (!result) {
      return;
    }
    const message = formatQuotaRefreshResult(result, oauthProviderLabel(provider));
    if (result.failed === 0) {
      notify.success(message);
    } else if (result.failed === result.total) {
      notify.danger(message);
    } else {
      notify.warning(message);
    }
  }

  const toolbarStart = (
    <p aria-label="账号数量" className="text-[12px] text-secondary">
      共 <span className="tabular-nums">{kindAccounts.length}</span> 个账号
    </p>
  );

  const toolbarEnd = (
    <>
      {selectedProvider !== "claude" ? (
        <Button
          variant="ghost"
          disabled={
            quotaRefresh.pending || accounts.isFetching || kindAccounts.length === 0
          }
          onClick={() => void refreshAllQuotas()}
        >
          <RefreshCw
            size={14}
            className={quotaRefresh.pending ? "animate-spin" : undefined}
          />
          {quotaRefresh.pending ? "正在刷新额度" : "刷新全部额度"}
        </Button>
      ) : null}
      <Button
        variant="ghost"
        disabled={accounts.isFetching || !accounts.data}
        onClick={() => void accounts.refetch()}
      >
        <RefreshCw size={14} className={accounts.isFetching ? "animate-spin" : undefined} />
        刷新
      </Button>
      <Button variant="secondary" disabled={!accounts.data} onClick={openImport}>
        <Upload size={14} aria-hidden="true" />
        导入 JSON
      </Button>
      <Button variant="primary" disabled={!accounts.data} onClick={openLogin}>
        <LogIn size={14} aria-hidden="true" />
        OAuth认证
      </Button>
    </>
  );

  if (accounts.isPending && !accounts.data) {
    return (
      <KindSplitLayout
        aria-busy="true"
        toolbarStart={toolbarStart}
        toolbarEnd={toolbarEnd}
        kindNav={
          <OAuthProviderNav
            selected={selectedProvider}
            counts={counts}
            onSelect={selectProvider}
          />
        }
      >
        <div className="flex min-h-48 items-center justify-center text-sm text-secondary">
          正在读取 OAuth 账号
        </div>
      </KindSplitLayout>
    );
  }

  if (!accounts.data) {
    return (
      <KindSplitLayout
        toolbarStart={toolbarStart}
        toolbarEnd={toolbarEnd}
        kindNav={
          <OAuthProviderNav
            selected={selectedProvider}
            counts={counts}
            onSelect={selectProvider}
          />
        }
      >
        <Surface className="p-6" role="alert">
          <p className="font-semibold">无法读取 OAuth 账号</p>
          <p className="mt-2 text-sm text-secondary">{getOAuthErrorMessage(accounts.error)}</p>
          <Button className="mt-5" onClick={() => void accounts.refetch()} disabled={accounts.isFetching}>
            <RefreshCw size={14} className={accounts.isFetching ? "animate-spin" : undefined} />
            重试
          </Button>
        </Surface>
      </KindSplitLayout>
    );
  }

  const configuration = accounts.data;

  return (
    <>
      <KindSplitLayout
        aria-busy={accounts.isFetching}
        toolbarStart={toolbarStart}
        toolbarEnd={toolbarEnd}
        kindNav={
          <OAuthProviderNav
            selected={selectedProvider}
            counts={counts}
            onSelect={selectProvider}
          />
        }
      >
        {accounts.isError ? (
          <Surface
            className="mb-3 flex flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between"
            role="status"
          >
            <p className="text-sm text-secondary">
              配置刷新失败，当前仍显示最近一次有效数据：{getOAuthErrorMessage(accounts.error)}
            </p>
            <Button onClick={() => void accounts.refetch()} disabled={accounts.isFetching}>
              重新加载
            </Button>
          </Surface>
        ) : null}

        <OAuthAccounts
          provider={selectedProvider}
          accounts={kindAccounts}
          configRevision={configuration.configRevision}
          quotaRefreshPending={quotaRefresh.pending}
        />
      </KindSplitLayout>

      <OAuthLoginDrawer
        open={loginOpen && !login.completedAccount}
        provider={selectedProvider}
        session={login.session}
        pending={login.pending}
        error={login.error}
        onClose={closeLogin}
        onRestart={() => {
          void login.start(selectedProvider).catch(() => {
            // Drawer keeps the safe user-facing error.
          });
        }}
        onExchange={async (callbackUrl) => {
          await login.exchange(callbackUrl);
          setLoginOpen(false);
        }}
      />
      {importOpen ? (
        <OAuthImportDrawer
          onClose={() => setImportOpen(false)}
          onImported={async (result) => {
            await accounts.refetch();
            notify.success(`已导入并启用 ${result.importedCount} 个 OAuth 账号。`);
          }}
        />
      ) : null}
    </>
  );
}

function resolveSelectedProvider(value: string | null): OAuthProvider {
  if (isOAuthProvider(value)) {
    return value;
  }
  return OAUTH_PROVIDER_OPTIONS[0]?.provider ?? "codex";
}

function formatQuotaRefreshResult(
  result: { total: number; failed: number },
  providerName: string,
) {
  if (result.failed === 0) {
    return `已刷新全部 ${result.total} 个 ${providerName} 账号额度。`;
  }
  if (result.failed === result.total) {
    return `全部 ${result.total} 个 ${providerName} 账号额度刷新失败。`;
  }
  return `已刷新 ${result.total - result.failed} 个 ${providerName} 账号额度，${result.failed} 个失败。`;
}
