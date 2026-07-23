import { RefreshCw } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { ProxyProfile } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { useProxies } from "../model/use-proxies";
import { useProxyAuthenticationActions } from "../model/use-proxy-authentication-actions";
import type { ProxyEditorSubmit } from "../model/use-proxy-editor";
import { useProxyMutations } from "../model/use-proxy-mutations";
import { ProxyEditor } from "./ProxyEditor";
import { ProxyList } from "./ProxyList";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { Surface } from "@/shared/ui/Surface";

export function ProxyManagement() {
  const proxies = useProxies();
  const mutations = useProxyMutations();
  const authentication = useProxyAuthenticationActions();
  const [searchParams, setSearchParams] = useSearchParams();
  const [deleteTarget, setDeleteTarget] = useState<ProxyProfile | null>(null);
  const editorId = searchParams.get("editor");
  const confirmPending = mutations.remove.isPending;

  function openEditor(id: string) {
    setDeleteTarget(null);
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
        正在读取出口代理配置
      </div>
    );
  }
  if (!proxies.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取出口代理配置</p>
        <p className="mt-2 text-sm text-secondary">{getProxyErrorMessage(proxies.error)}</p>
        <Button className="mt-5" onClick={() => void proxies.refetch()} disabled={proxies.isFetching}>
          <RefreshCw size={14} className={proxies.isFetching ? "animate-spin" : undefined} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = proxies.data;
  const selectedCandidate =
    editorId && editorId !== "new"
      ? configuration.items.find((proxy) => proxy.id === editorId)
      : undefined;
  const selected = selectedCandidate?.builtIn ? undefined : selectedCandidate;
  const directEditor = Boolean(selectedCandidate?.builtIn);
  const invalidEditor = editorId !== null && editorId !== "new" && !selected;
  const editorOpen = editorId !== null;
  const editorError =
    authentication.error ??
    (editorId === "new" ? mutations.create.error : mutations.update.error);
  const editorPending =
    mutations.create.isPending ||
    mutations.update.isPending ||
    authentication.pending;

  async function submitEditor(submit: ProxyEditorSubmit) {
    const nextConfiguration =
      editorId === "new"
        ? await mutations.create.mutateAsync(submit.input)
        : selected
          ? await mutations.update.mutateAsync({ id: selected.id, input: submit.input })
          : null;

    if (!nextConfiguration) {
      return;
    }

    const proxyId =
      editorId === "new"
        ? [...nextConfiguration.items]
            .reverse()
            .find((item) => !item.builtIn && item.name === submit.input.name)?.id
        : selected?.id;

    if (!proxyId) {
      closeEditor(editorId);
      return;
    }

    const current = nextConfiguration.items.find((item) => item.id === proxyId);

    if (submit.auth.kind === "disabled") {
      if (current?.passwordConfigured) {
        await authentication.clear(proxyId, nextConfiguration.configRevision);
      }
      closeEditor(editorId);
      return;
    }

    if (submit.auth.kind === "set") {
      await authentication.set(proxyId, nextConfiguration.configRevision, {
        username: submit.auth.username,
        password: submit.auth.password,
      });
    }

    closeEditor(editorId);
  }

  function requestDelete(proxy: ProxyProfile) {
    mutations.remove.reset();
    setDeleteTarget(proxy);
  }

  async function confirmDelete() {
    if (!deleteTarget) {
      return;
    }
    try {
      await mutations.remove.mutateAsync({
        id: deleteTarget.id,
        expectedRevision: configuration.configRevision,
      });
      setDeleteTarget(null);
    } catch {
      // Keep confirmation visible when the version is stale.
    }
  }

  function refreshData() {
    void proxies.refetch();
  }

  const drawerTitle = directEditor
    ? "DIRECT 不可编辑"
    : invalidEditor
      ? "出口代理不存在"
      : editorId === "new"
        ? "新增"
        : "编辑出口代理";
  const drawerDescription = directEditor
    ? "DIRECT 是系统内置出口，始终启用。"
    : invalidEditor
      ? "该链接可能已经过期，请返回列表。"
      : "HTTP 与 SOCKS5 使用独立出口配置";
  const deleteEndpoint =
    deleteTarget?.host && deleteTarget.port
      ? `${deleteTarget.host}:${deleteTarget.port}`
      : null;

  return (
    <div aria-busy={editorPending || mutations.isPending || proxies.isFetching}>
      {proxies.isError ? (
        <Surface
          className="mb-5 flex flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between"
          role="status"
        >
          <p className="text-sm text-secondary">
            配置刷新失败，当前仍显示最近一次有效数据：{getProxyErrorMessage(proxies.error)}
          </p>
          <Button onClick={refreshData} disabled={proxies.isFetching}>
            重新加载
          </Button>
        </Surface>
      ) : null}

      <ProxyList
        configuration={configuration}
        pending={mutations.isPending}
        refreshing={proxies.isFetching}
        actionError={mutations.remove.error}
        onCreate={() => openEditor("new")}
        onRefresh={refreshData}
        onEdit={openEditor}
        onDelete={requestDelete}
      />

      <SideDrawer
        open={editorOpen}
        title={drawerTitle}
        description={drawerDescription}
        onClose={() => closeEditor(editorId)}
      >
        {directEditor || invalidEditor ? (
          <div className="space-y-4 text-sm text-secondary">
            <p>{directEditor ? "请选择其他自定义出口代理进行编辑。" : "可以从出口代理列表重新进入。"}</p>
            <Button onClick={() => closeEditor(editorId)}>返回列表</Button>
          </div>
        ) : (
          <ProxyEditor
            key={editorId}
            profile={selected}
            isGlobal={selected?.id === configuration.globalProxyId}
            configRevision={configuration.configRevision}
            pending={editorPending}
            error={editorError}
            onSubmit={submitEditor}
            onClose={() => closeEditor(editorId)}
          />
        )}
      </SideDrawer>

      <ConfirmDialog
        open={deleteTarget !== null}
        title={deleteTarget ? `删除「${deleteTarget.name}」？` : ""}
        description={
          deleteTarget
            ? deleteEndpoint
              ? `将删除 ${deleteTarget.kind.toUpperCase()} 出口代理 ${deleteEndpoint}。绑定它的凭据需要改选其他出口。`
              : "此操作不可恢复。绑定它的凭据需要改选其他出口。"
            : undefined
        }
        confirmLabel="删除"
        tone="danger"
        pending={confirmPending}
        onConfirm={() => void confirmDelete()}
        onClose={() => {
          if (!confirmPending) {
            setDeleteTarget(null);
          }
        }}
      />
    </div>
  );
}
