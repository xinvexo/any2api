import { useState } from "react";

import type { ProviderEndpoint, ProviderEndpointWriteInput } from "../api/provider-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderRouteState } from "../model/provider-route-state";
import { useProviderEndpointMutations } from "../model/use-provider-mutations";
import { useProviderEndpoints } from "../model/use-providers";
import { ProviderEditorSlot } from "./ProviderEditorSlot";
import { ProviderEndpointList } from "./ProviderEndpointList";
import { notify } from "@/shared/notifications";
import { providerKindLabel } from "@/shared/api/provider-protocol-vocabulary";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { Surface } from "@/shared/ui/Surface";

export function ProviderManagement() {
  const endpoints = useProviderEndpoints();
  const mutations = useProviderEndpointMutations();
  const routeState = useProviderRouteState();
  const [deleteTarget, setDeleteTarget] = useState<ProviderEndpoint | null>(null);
  const editorId = routeState.endpointEditorId;
  const selectedKind = routeState.selectedKind;

  async function refreshEndpoints() {
    const result = await endpoints.refetch();
    if (result.isSuccess) {
      notify.success("Provider Endpoint 已刷新");
    }
  }

  function openEditor(id: string, kind = selectedKind) {
    mutations.create.reset();
    mutations.update.reset();
    routeState.openEndpointEditor(id, kind);
  }

  function closeEditor(expectedId: string | null = editorId) {
    mutations.create.reset();
    mutations.update.reset();
    routeState.closeEndpointEditor(expectedId);
  }

  const configuration = endpoints.data;
  const selected =
    configuration && editorId && editorId !== "new"
      ? configuration.items.find((endpoint) => endpoint.id === editorId)
      : undefined;
  const editorOpen = configuration !== undefined && editorId !== null;
  const editorError = editorId === "new" ? mutations.create.error : mutations.update.error;
  const editorPending = mutations.create.isPending || mutations.update.isPending;
  const editorKindName = providerKindLabel(selected?.providerKind ?? selectedKind);

  async function submitEditor(input: ProviderEndpointWriteInput) {
    if (!configuration) {
      return;
    }
    if (editorId === "new") {
      await mutations.create.mutateAsync(input);
      notify.success(`已创建「${input.name}」`);
    } else if (selected) {
      await mutations.update.mutateAsync({ id: selected.id, input });
      notify.success(`已保存「${input.name}」`);
    } else {
      return;
    }
    closeEditor(editorId);
  }

  function toggleEndpoint(endpoint: ProviderEndpoint) {
    if (!configuration) {
      return;
    }
    const nextEnabled = !endpoint.enabled;
    mutations.update.reset();
    mutations.update.mutate(
      {
        id: endpoint.id,
        input: {
          expectedRevision: configuration.configRevision,
          expectedConfigVersion: endpoint.configVersion,
          name: endpoint.name,
          providerKind: endpoint.providerKind,
          baseUrl: endpoint.baseUrl,
          protocolDialect: endpoint.protocolDialect,
          upstreamProtocolDialect: endpoint.upstreamProtocolDialect,
          enabled: nextEnabled,
        },
      },
      {
        onSuccess: () => {
          notify.success(nextEnabled ? `已启用「${endpoint.name}」` : `已停用「${endpoint.name}」`);
        },
        onError: (error) => {
          notify.danger(getProviderErrorMessage(error));
        },
      },
    );
  }

  function confirmDelete() {
    if (!configuration || !deleteTarget) {
      return;
    }
    const target = deleteTarget;
    mutations.remove.reset();
    mutations.remove.mutate(
      { id: target.id, expectedRevision: configuration.configRevision },
      {
        onSuccess: () => {
          notify.success(`已删除「${target.name}」`);
          setDeleteTarget(null);
        },
        onError: (error) => {
          notify.danger(getProviderErrorMessage(error));
          setDeleteTarget(null);
        },
      },
    );
  }

  return (
    <div
      className="flex h-full min-h-0 flex-col"
      aria-busy={editorPending || mutations.isPending || endpoints.isFetching}
    >
      {configuration && endpoints.isError ? (
        <Surface
          className="mb-5 flex flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between"
          role="status"
        >
          <p className="text-sm text-secondary">
            配置刷新失败，当前仍显示最近一次有效数据：{getProviderErrorMessage(endpoints.error)}
          </p>
          <Button onClick={() => void refreshEndpoints()} disabled={endpoints.isFetching}>
            重新加载
          </Button>
        </Surface>
      ) : null}

      <ProviderEndpointList
        items={configuration?.items ?? []}
        loadState={
          endpoints.isPending && !configuration
            ? { kind: "loading" }
            : !configuration
              ? {
                  kind: "error",
                  message: getProviderErrorMessage(endpoints.error),
                  refreshing: endpoints.isFetching,
                  onRetry: () => void refreshEndpoints(),
                }
              : undefined
        }
        pending={mutations.isPending}
        refreshing={endpoints.isFetching}
        onCreate={(kind) => openEditor("new", kind)}
        onRefresh={() => void refreshEndpoints()}
        onEdit={openEditor}
        onToggleEnabled={toggleEndpoint}
        onDelete={setDeleteTarget}
      />

      <SideDrawer
        open={editorOpen}
        title={editorId === "new" ? "新增" : "编辑 Endpoint"}
        description={`配置 ${editorKindName} 上游地址`}
        onClose={() => closeEditor(editorId)}
      >
        {editorId && configuration ? (
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
            ? `确定删除「${deleteTarget.name}」？绑定的 API Key、模型权限及对应路由目标也会一并移除。`
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
