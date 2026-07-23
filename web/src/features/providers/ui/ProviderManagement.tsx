import { RefreshCw } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import type {
  ProviderEndpoint,
  ProviderEndpointConfiguration,
  ProviderEndpointWriteInput,
  ProviderKind,
} from "../api/provider-contracts";
import { isProviderKind } from "../model/provider-kind-catalog";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderEndpointMutations } from "../model/use-provider-mutations";
import { useProviderEndpoints } from "../model/use-providers";
import { ProviderEndpointEditor } from "./ProviderEndpointEditor";
import { ProviderEndpointList } from "./ProviderEndpointList";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { Surface } from "@/shared/ui/Surface";

export function ProviderManagement() {
  const endpoints = useProviderEndpoints();
  const mutations = useProviderEndpointMutations();
  const [searchParams, setSearchParams] = useSearchParams();
  const [deleteTarget, setDeleteTarget] = useState<ProviderEndpoint | null>(null);
  const editorId = searchParams.get("editor");

  function openEditor(id: string, kind?: ProviderKind) {
    mutations.create.reset();
    mutations.update.reset();
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.delete("keys");
        next.delete("credential");
        next.delete("action");
        next.set("editor", id);
        if (kind) {
          next.set("kind", kind);
        }
        return next;
      },
      { replace: true },
    );
  }

  function closeEditor(expectedId: string | null = editorId) {
    mutations.create.reset();
    mutations.update.reset();
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

  if (endpoints.isPending && !endpoints.data) {
    return (
      <div className="flex min-h-56 items-center justify-center text-sm text-secondary" aria-busy="true">
        正在读取 Provider 配置
      </div>
    );
  }

  if (!endpoints.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取 Provider 配置</p>
        <p className="mt-2 text-sm text-secondary">{getProviderErrorMessage(endpoints.error)}</p>
        <Button className="mt-5" onClick={() => void endpoints.refetch()} disabled={endpoints.isFetching}>
          <RefreshCw size={14} className={endpoints.isFetching ? "animate-spin" : undefined} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = endpoints.data;
  const selected =
    editorId && editorId !== "new"
      ? configuration.items.find((endpoint) => endpoint.id === editorId)
      : undefined;
  const editorOpen = editorId !== null;
  const editorError = editorId === "new" ? mutations.create.error : mutations.update.error;
  const editorPending =
    mutations.create.isPending || mutations.update.isPending;

  async function submitEditor(input: ProviderEndpointWriteInput) {
    if (editorId === "new") {
      await mutations.create.mutateAsync(input);
    } else if (selected) {
      await mutations.update.mutateAsync({ id: selected.id, input });
    } else {
      return;
    }
    closeEditor(editorId);
  }

  function confirmDelete() {
    if (!deleteTarget) {
      return;
    }
    mutations.remove.reset();
    mutations.remove.mutate(
      { id: deleteTarget.id, expectedRevision: configuration.configRevision },
      {
        onSettled: () => setDeleteTarget(null),
      },
    );
  }

  const drawerTitle =
    editorId === "new" ? "新增" : "编辑 Endpoint";
  const kindParam = searchParams.get("kind");
  const selectedKind: ProviderKind = isProviderKind(kindParam) ? kindParam : "codex";
  const drawerDescription = "配置上游地址";

  return (
    <div aria-busy={editorPending || mutations.isPending || endpoints.isFetching}>
      {endpoints.isError ? (
        <Surface
          className="mb-5 flex flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between"
          role="status"
        >
          <p className="text-sm text-secondary">
            配置刷新失败，当前仍显示最近一次有效数据：{getProviderErrorMessage(endpoints.error)}
          </p>
          <Button onClick={() => void endpoints.refetch()} disabled={endpoints.isFetching}>
            重新加载
          </Button>
        </Surface>
      ) : null}

      <ProviderEndpointList
        configuration={configuration}
        pending={mutations.isPending}
        refreshing={endpoints.isFetching}
        actionError={mutations.remove.error}
        onCreate={(kind) => openEditor("new", kind)}
        onRefresh={() => void endpoints.refetch()}
        onEdit={openEditor}
        onDelete={setDeleteTarget}
      />

      <SideDrawer
        open={editorOpen}
        title={drawerTitle}
        description={drawerDescription}
        onClose={() => closeEditor(editorId)}
      >
        {editorId ? (
          <ProviderEditorSlot
            key={`${editorId}:${selectedKind}`}
            editorId={editorId}
            currentEndpoint={selected}
            defaultKind={selectedKind}
            protocolOptions={configuration.protocolOptions}
            configRevision={configuration.configRevision}
            pending={editorPending}
            error={editorError}
            onSubmit={submitEditor}
            onClose={() => closeEditor(editorId)}
          />
        ) : null}
      </SideDrawer>

      <ConfirmDialog
        open={deleteTarget !== null}
        title="删除 Endpoint"
        description={
          deleteTarget
            ? `确定删除「${deleteTarget.name}」？绑定的 API Key 也会一并移除。`
            : undefined
        }
        confirmLabel="删除"
        tone="danger"
        pending={mutations.remove.isPending}
        onConfirm={confirmDelete}
        onClose={() => {
          if (!mutations.remove.isPending) {
            setDeleteTarget(null);
          }
        }}
      />
    </div>
  );
}

function ProviderEditorSlot({
  editorId,
  currentEndpoint,
  defaultKind,
  protocolOptions,
  configRevision,
  pending,
  error,
  onSubmit,
  onClose,
}: {
  editorId: string;
  currentEndpoint?: ProviderEndpoint;
  defaultKind: ProviderKind;
  protocolOptions: ProviderEndpointConfiguration["protocolOptions"];
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSubmit: (input: ProviderEndpointWriteInput) => Promise<void>;
  onClose: () => void;
}) {
  const editing = editorId !== "new";
  const [initialEndpoint] = useState(currentEndpoint);

  if (editing && !initialEndpoint) {
    return (
      <div className="space-y-4 text-sm text-secondary">
        <p>Endpoint 不存在，该链接可能已经过期。</p>
        <Button onClick={onClose}>返回列表</Button>
      </div>
    );
  }

  const sourceConflict = editing
    ? !currentEndpoint
      ? "deleted"
      : currentEndpoint.configVersion !== initialEndpoint?.configVersion
        ? "changed"
        : null
    : null;

  return (
    <ProviderEndpointEditor
      endpoint={initialEndpoint}
      defaultKind={defaultKind}
      protocolOptions={protocolOptions}
      sourceConflict={sourceConflict}
      configRevision={configRevision}
      pending={pending}
      error={error}
      onSubmit={onSubmit}
      onClose={onClose}
    />
  );
}
