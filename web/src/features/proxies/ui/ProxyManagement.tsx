import { Plus, RefreshCw, Router } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import { useProviderEndpoints } from "@/features/providers";
import type { ProxyWriteInput } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { useProxies } from "../model/use-proxies";
import { useProxyAuthenticationActions } from "../model/use-proxy-authentication-actions";
import { useProxyMutations } from "../model/use-proxy-mutations";
import { useProxyTest } from "../model/use-proxy-test";
import { GlobalProxyPanel } from "./GlobalProxyPanel";
import { ProxyAuthenticationPanel } from "./ProxyAuthenticationPanel";
import { ProxyEditor } from "./ProxyEditor";
import { ProxyList } from "./ProxyList";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function ProxyManagement() {
  const proxies = useProxies();
  const providerEndpoints = useProviderEndpoints();
  const mutations = useProxyMutations();
  const authentication = useProxyAuthenticationActions();
  const proxyTest = useProxyTest(
    `${proxies.data?.configRevision ?? 0}:${providerEndpoints.data?.configRevision ?? 0}`,
  );
  const [requestedTestEndpointId, setRequestedTestEndpointId] = useState("");
  const [searchParams, setSearchParams] = useSearchParams();
  const editorId = searchParams.get("editor");

  function openEditor(id: string) {
    mutations.create.reset();
    mutations.update.reset();
    authentication.reset();
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.set("editor", id);
        return next;
      },
      { replace: true },
    );
  }

  function closeEditor(expectedId: string | null = editorId) {
    mutations.create.reset();
    mutations.update.reset();
    authentication.reset();
    setSearchParams(
      (current) => {
        if (expectedId && current.get("editor") !== expectedId) {
          return current;
        }
        const next = new URLSearchParams(current);
        next.delete("editor");
        return next;
      },
      { replace: true },
    );
  }

  if (proxies.isPending && !proxies.data) {
    return (
      <Surface className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary" aria-busy="true">
        正在读取代理配置
      </Surface>
    );
  }
  if (!proxies.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取代理配置</p>
        <p className="mt-2 text-sm text-secondary">{getProxyErrorMessage(proxies.error)}</p>
        <Button className="mt-5" onClick={() => void proxies.refetch()} disabled={proxies.isFetching}>
          <RefreshCw size={15} className={proxies.isFetching ? "animate-spin" : undefined} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = proxies.data;
  const endpoints = providerEndpoints.data?.items ?? [];
  const testEndpointId = endpoints.some((endpoint) => endpoint.id === requestedTestEndpointId)
    ? requestedTestEndpointId
    : endpoints.find((endpoint) => endpoint.enabled)?.id ?? endpoints[0]?.id ?? "";
  const selectedCandidate = editorId && editorId !== "new"
    ? configuration.items.find((proxy) => proxy.id === editorId)
    : undefined;
  const selected = selectedCandidate?.builtIn ? undefined : selectedCandidate;
  const directEditor = Boolean(selectedCandidate?.builtIn);
  const invalidEditor = editorId !== null && editorId !== "new" && !selected;
  const editorError = editorId === "new" ? mutations.create.error : mutations.update.error;

  async function submitEditor(input: ProxyWriteInput) {
    if (editorId === "new") {
      await mutations.create.mutateAsync(input);
      return;
    }
    if (selected) {
      await mutations.update.mutateAsync({ id: selected.id, input });
    }
  }

  function setGlobal(id: string) {
    mutations.setGlobal.reset();
    mutations.setGlobal.mutate({ id, expectedRevision: configuration.configRevision });
  }

  function remove(id: string) {
    mutations.remove.reset();
    mutations.remove.mutate({ id, expectedRevision: configuration.configRevision });
  }

  function test(id: string) {
    if (testEndpointId) {
      void proxyTest.test(id, testEndpointId);
    }
  }

  function refreshData() {
    void Promise.all([proxies.refetch(), providerEndpoints.refetch()]);
  }

  return (
    <div
      className="space-y-5"
      aria-busy={mutations.isPending || authentication.pending || proxies.isFetching}
    >
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-secondary">
          配置版本 <span className="font-medium tabular-nums text-primary">{configuration.configRevision}</span>
        </p>
        <div className="flex flex-col gap-2 sm:flex-row">
          <Button
            variant="ghost"
            onClick={refreshData}
            disabled={proxies.isFetching || providerEndpoints.isFetching}
          >
            <RefreshCw
              size={15}
              className={proxies.isFetching || providerEndpoints.isFetching ? "animate-spin" : undefined}
            />
            刷新
          </Button>
          <Button variant="primary" onClick={() => openEditor("new")} disabled={mutations.isPending}>
            <Plus size={16} />
            新增代理
          </Button>
        </div>
      </div>

      {proxies.isError ? (
        <Surface className="flex flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between" role="status">
          <p className="text-sm text-secondary">
            配置刷新失败，当前仍显示最近一次有效数据：{getProxyErrorMessage(proxies.error)}
          </p>
          <Button
            onClick={refreshData}
            disabled={proxies.isFetching || providerEndpoints.isFetching}
          >
            重新加载
          </Button>
        </Surface>
      ) : null}

      <GlobalProxyPanel
        key={configuration.configRevision}
        configuration={configuration}
        pending={mutations.isPending}
        error={mutations.setGlobal.error}
        onApply={setGlobal}
      />

      <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(320px,0.72fr)] lg:items-start">
        <div className={editorId ? "order-2 lg:order-1" : undefined}>
          <ProxyList
            configuration={configuration}
            pending={mutations.isPending}
            actionError={mutations.remove.error}
            onEdit={openEditor}
            onSetGlobal={setGlobal}
            onDelete={remove}
            endpoints={endpoints}
            testEndpointId={testEndpointId}
            testingProxyId={proxyTest.testingProxyId}
            testResults={proxyTest.results}
            onTestEndpointChange={setRequestedTestEndpointId}
            onTest={test}
            testError={proxyTest.error ?? providerEndpoints.error}
          />
        </div>

        <div className={editorId ? "order-1 lg:order-2" : undefined}>
          {editorId && !invalidEditor ? (
            <div className="space-y-5">
              <ProxyEditor
                key={editorId}
                profile={selected}
                isGlobal={selected?.id === configuration.globalProxyId}
                configRevision={configuration.configRevision}
                pending={editorId === "new" ? mutations.create.isPending : mutations.update.isPending}
                error={editorError}
                onSubmit={submitEditor}
                onClose={() => closeEditor(editorId)}
              />
              {selected ? (
                <ProxyAuthenticationPanel
                  key={`${selected.id}:${selected.authenticationVersion}`}
                  profile={selected}
                  configRevision={configuration.configRevision}
                  pending={authentication.pending}
                  error={authentication.error}
                  onSet={authentication.set}
                  onClear={authentication.clear}
                />
              ) : null}
            </div>
          ) : (
            <Surface className="flex min-h-52 items-center justify-center p-7 text-center lg:sticky lg:top-24">
              <div>
                <Router size={22} className="mx-auto text-tertiary" aria-hidden="true" />
                <p className="mt-3 text-sm font-medium">
                  {directEditor ? "DIRECT 不可编辑" : invalidEditor ? "代理不存在" : "选择一个代理进行编辑"}
                </p>
                <p className="mt-1 text-sm text-secondary">
                  {directEditor
                    ? "DIRECT 是系统内置出口，始终启用。"
                    : invalidEditor
                      ? "该链接可能已经过期，请返回列表。"
                      : "也可以新增 HTTP 或 SOCKS5 出口。"}
                </p>
                {invalidEditor ? (
                  <Button className="mt-4" onClick={() => closeEditor(editorId)}>
                    返回列表
                  </Button>
                ) : null}
              </div>
            </Surface>
          )}
        </div>
      </div>
    </div>
  );
}
