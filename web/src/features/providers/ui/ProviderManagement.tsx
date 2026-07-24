import { Plus, RefreshCw, Search } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { useSearchParams } from "react-router-dom";

import type { ProviderEndpoint, ProviderEndpointWriteInput, ProviderKind } from "../api/provider-contracts";
import {
  isProviderKind,
  PROVIDER_KIND_OPTIONS,
  providerKindLabel,
} from "../model/provider-kind-catalog";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderEndpointMutations } from "../model/use-provider-mutations";
import { useProviderEndpoints } from "../model/use-providers";
import { ProviderEditorSlot } from "./ProviderEditorSlot";
import { ProviderEndpointList } from "./ProviderEndpointList";
import { ProviderKindNav } from "./ProviderKindNav";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { KindSplitLayout } from "@/shared/ui/KindSplitLayout";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { Surface } from "@/shared/ui/Surface";

export function ProviderManagement() {
  const endpoints = useProviderEndpoints();
  const mutations = useProviderEndpointMutations();
  const [searchParams, setSearchParams] = useSearchParams();
  const [deleteTarget, setDeleteTarget] = useState<ProviderEndpoint | null>(null);
  const editorId = searchParams.get("editor");
  const kindParam = searchParams.get("kind");
  const selectedKind: ProviderKind = isProviderKind(kindParam) ? kindParam : "codex";
  const kindName = providerKindLabel(selectedKind);
  const emptyCounts = useMemo(
    () =>
      Object.fromEntries(PROVIDER_KIND_OPTIONS.map((option) => [option.kind, 0])) as Record<
        ProviderKind,
        number
      >,
    [],
  );

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

  function selectKind(kind: ProviderKind) {
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.set("kind", kind);
        next.delete("keys");
        next.delete("credential");
        next.delete("action");
        next.delete("editor");
        return next;
      },
      { replace: true },
    );
  }

  if (endpoints.isPending && !endpoints.data) {
    return (
      <ProviderChrome
        kindName={kindName}
        selectedKind={selectedKind}
        counts={emptyCounts}
        onSelectKind={selectKind}
        busy
        onRefresh={() => undefined}
        refreshing={false}
        canCreate={false}
        onCreate={() => undefined}
      >
        <div className="flex min-h-48 items-center justify-center text-sm text-secondary">
          正在读取 Provider 配置
        </div>
      </ProviderChrome>
    );
  }

  if (!endpoints.data) {
    return (
      <ProviderChrome
        kindName={kindName}
        selectedKind={selectedKind}
        counts={emptyCounts}
        onSelectKind={selectKind}
        onRefresh={() => void endpoints.refetch()}
        refreshing={endpoints.isFetching}
        canCreate={false}
        onCreate={() => undefined}
      >
        <Surface className="p-6" role="alert">
          <p className="font-semibold">无法读取 Provider 配置</p>
          <p className="mt-2 text-sm text-secondary">{getProviderErrorMessage(endpoints.error)}</p>
          <Button className="mt-5" onClick={() => void endpoints.refetch()} disabled={endpoints.isFetching}>
            <RefreshCw size={14} className={endpoints.isFetching ? "animate-spin" : undefined} />
            重试
          </Button>
        </Surface>
      </ProviderChrome>
    );
  }

  const configuration = endpoints.data;
  const selected =
    editorId && editorId !== "new"
      ? configuration.items.find((endpoint) => endpoint.id === editorId)
      : undefined;
  const editorOpen = editorId !== null;
  const editorError = editorId === "new" ? mutations.create.error : mutations.update.error;
  const editorPending = mutations.create.isPending || mutations.update.isPending;

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
      { onSettled: () => setDeleteTarget(null) },
    );
  }

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
        title={editorId === "new" ? "新增" : "编辑 Endpoint"}
        description="配置上游地址"
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

function ProviderChrome({
  kindName,
  selectedKind,
  counts,
  onSelectKind,
  busy,
  onRefresh,
  refreshing,
  canCreate,
  onCreate,
  children,
}: {
  kindName: string;
  selectedKind: ProviderKind;
  counts: Record<ProviderKind, number>;
  onSelectKind: (kind: ProviderKind) => void;
  busy?: boolean;
  onRefresh: () => void;
  refreshing: boolean;
  canCreate: boolean;
  onCreate: () => void;
  children: ReactNode;
}) {
  return (
    <KindSplitLayout
      aria-busy={busy || undefined}
      toolbarStart={
        <>
          <Search
            size={14}
            className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-tertiary"
            aria-hidden="true"
          />
          <input
            className="focus-ring h-8 w-full rounded-[8px] border-0 bg-surface-muted py-0 pl-8 pr-3 text-[12px] text-primary placeholder:text-tertiary"
            disabled
            placeholder={`搜索 ${kindName} Endpoint`}
            aria-label={`搜索 ${kindName}`}
            value=""
            readOnly
          />
        </>
      }
      toolbarEnd={
        <>
          <Button variant="ghost" disabled={refreshing || busy} onClick={onRefresh}>
            <RefreshCw size={14} className={refreshing ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button variant="primary" disabled={!canCreate || busy} onClick={onCreate}>
            <Plus size={14} />
            新增
          </Button>
        </>
      }
      kindNav={<ProviderKindNav selected={selectedKind} counts={counts} onSelect={onSelectKind} />}
    >
      {children}
    </KindSplitLayout>
  );
}
