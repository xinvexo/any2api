import { LogIn, RefreshCw, Upload } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";

import type {
  OAuthActivationResult,
  OAuthProvider,
  OAuthProxySelection,
} from "../api/oauth-contracts";
import { getOAuthErrorMessage } from "../model/oauth-error";
import {
  isOAuthProvider,
  oauthProviderLabel,
  OAUTH_PROVIDER_OPTIONS,
} from "../model/oauth-provider-catalog";
import { useOAuthAccounts } from "../model/use-oauth-accounts";
import { useOAuthLogin } from "../model/use-oauth-login";
import { useOAuthQuotaRefreshAll } from "../model/use-oauth-quota-refresh-all";
import { useOAuthQuotaChangeEvent } from "../model/use-oauth-quota-change-event";
import { OAuthAccounts } from "./OAuthAccounts";
import { OAuthLoginDrawer } from "./OAuthLogin";
import { OAuthImportDrawer } from "./OAuthImport";
import { OAuthProviderNav } from "./OAuthProviderNav";
import { useProxyConfiguration } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";
import { KindSplitLayout } from "@/shared/ui/KindSplitLayout";
import { Surface } from "@/shared/ui/Surface";
import { notify } from "@/shared/notifications";

/** Shares KindSplitLayout with 上游提供 so route switches keep chrome geometry. */
export function OAuthManagement() {
  const accounts = useOAuthAccounts();
  const proxies = useProxyConfiguration();
  const login = useOAuthLogin();
  const quotaRefresh = useOAuthQuotaRefreshAll();
  useOAuthQuotaChangeEvent();
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedProvider = resolveSelectedProvider(searchParams.get("kind"));
  const [loginOpen, setLoginOpen] = useState(false);
  const [loginProxySelection, setLoginProxySelection] =
    useState<OAuthProxySelection>({ mode: "global" });
  const [importOpen, setImportOpen] = useState(false);
  const notifiedActivation = useRef<OAuthActivationResult | null>(null);

  useEffect(() => {
    const activation = login.completedAccount;
    if (!activation) {
      notifiedActivation.current = null;
      return;
    }
    if (notifiedActivation.current === activation) {
      return;
    }
    notifiedActivation.current = activation;
    setLoginOpen(false);
    notify.success(`已激活 OAuth 账号「${activation.label}」`);
  }, [login.completedAccount]);

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
    if (next === selectedProvider || quotaRefresh.pending || login.pending !== null) {
      return;
    }
    setLoginOpen(false);
    setImportOpen(false);
    setLoginProxySelection({ mode: "global" });
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
    login.reset();
    setLoginProxySelection({ mode: "global" });
    setLoginOpen(true);
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
    if (kindAccounts.length === 0) {
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

  async function refreshConfiguration() {
    const [accountResult, proxyResult] = await Promise.all([
      accounts.refetch(),
      proxies.refetch(),
    ]);
    if (accountResult.isSuccess && proxyResult.isSuccess) {
      notify.success("OAuth 配置已刷新");
    }
  }

  const toolbarStart = (
    <p aria-label="账号数量" className="text-[12px] text-secondary">
      共 <span className="tabular-nums">{kindAccounts.length}</span> 个账号
    </p>
  );

  const toolbarEnd = (
    <>
      <Button
        variant="ghost"
        aria-label="刷新全部额度"
        disabled={
          quotaRefresh.pending || accounts.isFetching || kindAccounts.length === 0
        }
        onClick={() => void refreshAllQuotas()}
      >
        <RefreshCw
          size={14}
          className={quotaRefresh.pending ? "animate-spin" : undefined}
        />
        {quotaRefresh.pending ? "刷新中" : "刷新额度"}
      </Button>
      <Button
        variant="ghost"
        disabled={accounts.isFetching || proxies.isFetching || !accounts.data}
        onClick={() => void refreshConfiguration()}
      >
        <RefreshCw
          size={14}
          className={accounts.isFetching || proxies.isFetching ? "animate-spin" : undefined}
        />
        刷新
      </Button>
      <Button
        variant="secondary"
        disabled={quotaRefresh.pending || !accounts.data || !proxies.data}
        onClick={openImport}
      >
        <Upload size={14} aria-hidden="true" />
        导入 JSON
      </Button>
      <Button
        variant="primary"
        disabled={quotaRefresh.pending || !accounts.data || !proxies.data}
        onClick={openLogin}
      >
        <LogIn size={14} aria-hidden="true" />
        OAuth认证
      </Button>
    </>
  );

  return (
    <>
      <KindSplitLayout
        aria-busy={accounts.isFetching || proxies.isFetching || undefined}
        toolbarStart={toolbarStart}
        toolbarEnd={toolbarEnd}
        kindNav={
          <OAuthProviderNav
            selected={selectedProvider}
            counts={counts}
            disabled={quotaRefresh.pending || login.pending !== null}
            onSelect={selectProvider}
          />
        }
      >
        {(accounts.isPending && !accounts.data) || (proxies.isPending && !proxies.data) ? (
          <div className="flex h-full min-h-48 items-center justify-center text-sm text-secondary">
            正在读取 OAuth 配置
          </div>
        ) : !accounts.data ? (
          <Surface className="p-6" role="alert">
            <p className="font-semibold">无法读取 OAuth 账号</p>
            <p className="mt-2 text-sm text-secondary">{getOAuthErrorMessage(accounts.error)}</p>
            <Button
              className="mt-5"
              onClick={() => void refreshConfiguration()}
              disabled={accounts.isFetching || proxies.isFetching}
            >
              <RefreshCw
                size={14}
                className={accounts.isFetching || proxies.isFetching ? "animate-spin" : undefined}
              />
              重试
            </Button>
          </Surface>
        ) : !proxies.data ? (
          <Surface className="p-6" role="alert">
            <p className="font-semibold">无法读取出口代理配置</p>
            <p className="mt-2 text-sm text-secondary">{getOAuthErrorMessage(proxies.error)}</p>
            <Button
              className="mt-5"
              onClick={() => void refreshConfiguration()}
              disabled={accounts.isFetching || proxies.isFetching}
            >
              <RefreshCw
                size={14}
                className={accounts.isFetching || proxies.isFetching ? "animate-spin" : undefined}
              />
              重试
            </Button>
          </Surface>
        ) : (
          <div className="flex h-full min-h-0 flex-col">
            {accounts.isError ? (
              <Surface
                className="mb-3 flex shrink-0 flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between"
                role="status"
              >
                <p className="text-sm text-secondary">
                  配置刷新失败，当前仍显示最近一次有效数据：{getOAuthErrorMessage(accounts.error)}
                </p>
                <Button
                  onClick={() => void refreshConfiguration()}
                  disabled={accounts.isFetching || proxies.isFetching}
                >
                  重新加载
                </Button>
              </Surface>
            ) : null}

            <OAuthAccounts
              provider={selectedProvider}
              accounts={kindAccounts}
              configRevision={accounts.data.configRevision}
              proxyConfiguration={proxies.data}
              quotaRefreshPending={quotaRefresh.pending}
            />
          </div>
        )}
      </KindSplitLayout>

      {accounts.data && proxies.data ? (
        <OAuthLoginDrawer
            open={loginOpen && !login.completedAccount}
            provider={selectedProvider}
            proxySelection={loginProxySelection}
            proxyConfiguration={proxies.data}
            session={login.session}
            pending={login.pending}
            error={login.error}
            onClose={closeLogin}
            onProxySelectionChange={setLoginProxySelection}
            onStart={() => {
              void login.start(selectedProvider, loginProxySelection).catch(() => {
                // Drawer keeps the safe user-facing error.
              });
            }}
            onExchange={async (callbackUrl) => {
              await login.exchange(callbackUrl);
              setLoginOpen(false);
            }}
        />
      ) : null}
      {accounts.data ? (
        <>
          {importOpen ? (
            <OAuthImportDrawer
              onClose={() => setImportOpen(false)}
              onReconcile={async () => {
                await accounts.refetch();
              }}
              onImported={async (result) => {
                await accounts.refetch();
                notify.success(`已导入并启用 ${result.importedCount} 个 OAuth 账号。`);
              }}
            />
          ) : null}
        </>
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

function formatQuotaRefreshResult(result: { total: number; failed: number }, providerName: string) {
  if (result.failed === 0) {
    return `已刷新全部 ${result.total} 个 ${providerName} 账号额度。`;
  }
  if (result.failed === result.total) {
    return `全部 ${result.total} 个 ${providerName} 账号额度刷新失败。`;
  }
  return `已刷新 ${result.total - result.failed} 个 ${providerName} 账号额度，${result.failed} 个失败。`;
}
