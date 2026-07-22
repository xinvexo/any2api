import { RefreshCw } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import { useProviderEndpoints } from "@/features/providers";
import type { ProxyWriteInput } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { useProxies } from "../model/use-proxies";
import { useProxyAuthenticationActions } from "../model/use-proxy-authentication-actions";
import { useProxyMutations } from "../model/use-proxy-mutations";
import { useProxyTest } from "../model/use-proxy-test";
import { ProxyAuthenticationPanel } from "./ProxyAuthenticationPanel";
import { ProxyEditor } from "./ProxyEditor";
import { ProxyList } from "./ProxyList";
import { Button } from "@/shared/ui/Button";
import { SideDrawer } from "@/shared/ui/SideDrawer";
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
      <div className="flex min-h-56 items-center justify-center text-sm text-secondary" aria-busy="true">
        正在读取代理配置
      </div>
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
  const selectedCandidate =
    editorId && editorId !== "new"
      ? configuration.items.find((proxy) => proxy.id === editorId)
      : undefined;
  const selected = selectedCandidate?.builtIn ? undefined : selectedCandidate;
  const directEditor = Boolean(selectedCandidate?.builtIn);
  const invalidEditor = editorId !== null && editorId !== "new" && !selected;
  const editorOpen = editorId !== null;
  const editorError = editorId === "new" ? mutations.create.error : mutations.update.error;

  async function submitEditor(input: ProxyWriteInput) {
    if (editorId === "new") {
      const configurationAfterCreate = await mutations.create.mutateAsync(input);
      const created = [...configurationAfterCreate.items]
        .reverse()
        .find((item) => !item.builtIn && item.name === input.name);
      if (created) {
        openEditor(created.id);
      } else {
        closeEditor("new");
      }
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

  const drawerTitle = directEditor
    ? "DIRECT 不可编辑"
    : invalidEditor
      ? "代理不存在"
      : editorId === "new"
        ? "新增代理"
        : "编辑代理";
  const drawerDescription = directEditor
    ? "DIRECT 是系统内置出口，始终启用。"
    : invalidEditor
      ? "该链接可能已经过期，请返回列表。"
      : "HTTP 与 SOCKS5 使用独立出口配置";

  return (
    <div aria-busy={mutations.isPending || authentication.pending || proxies.isFetching}>
      {proxies.isError ? (
        <Surface
          className="mb-5 flex flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between"
          role="status"
        >
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

      <ProxyList
        configuration={configuration}
        pending={mutations.isPending}
        refreshing={proxies.isFetching || providerEndpoints.isFetching}
        actionError={mutations.remove.error ?? mutations.setGlobal.error}
        onCreate={() => openEditor("new")}
        onRefresh={refreshData}
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

      <SideDrawer
        open={editorOpen}
        title={drawerTitle}
        description={drawerDescription}
        onClose={() => closeEditor(editorId)}
      >
        {directEditor || invalidEditor ? (
          <div className="space-y-4 text-sm text-secondary">
            <p>{directEditor ? "请选择其他自定义代理进行编辑。" : "可以从代理列表重新进入。"}</p>
            <Button onClick={() => closeEditor(editorId)}>返回列表</Button>
          </div>
        ) : (
          <>
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
          </>
        )}
      </SideDrawer>
    </div>
  );
}
