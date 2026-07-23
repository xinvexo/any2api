import { RefreshCw } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { getGatewayApiKeyErrorMessage } from "../model/gateway-api-key-error";
import { useGatewayApiKeyMutations } from "../model/use-gateway-api-key-mutations";
import { useGatewayApiKeySecretActions } from "../model/use-gateway-api-key-secret-actions";
import { useGatewayApiKeys } from "../model/use-gateway-api-keys";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { Surface } from "@/shared/ui/Surface";

import {
  GatewayApiKeyEditor,
  type GatewayApiKeyEditorSubmit,
} from "./GatewayApiKeyEditor";
import { GatewayApiKeyList } from "./GatewayApiKeyList";

export function GatewayApiKeyManagement() {
  const query = useGatewayApiKeys();
  const mutations = useGatewayApiKeyMutations();
  const secretActions = useGatewayApiKeySecretActions();
  const [searchParams, setSearchParams] = useSearchParams();
  const [deleteTarget, setDeleteTarget] = useState<GatewayApiKey | null>(null);
  const editorId = searchParams.get("editor");
  const selected =
    editorId && editorId !== "new"
      ? query.data?.items.find((key) => key.id === editorId)
      : undefined;
  const editorPending = mutations.update.isPending || secretActions.pending;
  const confirmPending = mutations.revoke.isPending;

  function openEditor(id: string) {
    setDeleteTarget(null);
    mutations.update.reset();
    secretActions.reset();
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
    mutations.update.reset();
    secretActions.reset();
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

  async function submitEditor(input: GatewayApiKeyEditorSubmit) {
    if (editorId === "new") {
      await secretActions.create({
        expectedRevision: query.data?.configRevision ?? 0,
        name: input.name,
        enabled: input.enabled,
      });
      closeEditor(editorId);
      return;
    }

    if (!selected || !query.data) {
      return;
    }

    let current = selected;
    let revision = query.data.configRevision;
    const metaChanged =
      current.name !== input.name || current.enabled !== input.enabled;

    if (metaChanged) {
      const configuration = await mutations.update.mutateAsync({
        id: current.id,
        input: {
          expectedRevision: revision,
          expectedConfigVersion: current.configVersion,
          name: input.name,
          enabled: input.enabled,
        },
      });
      revision = configuration.configRevision;
      const updated = configuration.items.find((item) => item.id === current.id);
      if (!updated) {
        closeEditor(editorId);
        return;
      }
      current = updated;
    }

    if (input.regenerateToken) {
      await secretActions.regenerate(current.id, {
        expectedRevision: revision,
        expectedConfigVersion: current.configVersion,
        expectedTokenVersion: current.tokenVersion,
      });
    }

    closeEditor(editorId);
  }

  function requestDelete(key: GatewayApiKey) {
    setDeleteTarget(key);
  }

  async function confirmDelete() {
    if (!deleteTarget || !query.data) {
      return;
    }
    try {
      await mutations.revoke.mutateAsync({
        id: deleteTarget.id,
        input: {
          expectedRevision: query.data.configRevision,
          expectedConfigVersion: deleteTarget.configVersion,
        },
      });
      setDeleteTarget(null);
    } catch {
      // Keep confirmation visible when the version is stale.
    }
  }

  if (query.isPending && !query.data) {
    return (
      <div className="flex min-h-56 items-center justify-center text-sm text-secondary" aria-live="polite">
        正在加载网关密钥…
      </div>
    );
  }

  if (query.isError && !query.data) {
    return (
      <Surface className="space-y-4 p-5">
        <p className="text-sm text-danger" role="alert">
          {getGatewayApiKeyErrorMessage(query.error)}
        </p>
        <Button onClick={() => void query.refetch()}>
          <RefreshCw size={15} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = query.data;
  if (!configuration) {
    return null;
  }

  const editorOpen = editorId !== null;
  const editorInvalid = editorId !== null && editorId !== "new" && !selected;
  const drawerTitle =
    editorId === "new" ? "新增" : selected ? `编辑「${selected.name}」` : "密钥不存在";
  const drawerDescription =
    editorId === "new"
      ? "创建后密钥会明文保存在本机配置中，可随时在列表查看。"
      : "可修改名称与启用状态；重新生成会立即替换旧密钥。";
  const editorError = mutations.update.error ?? secretActions.error;

  return (
    <div className="space-y-4">
      {query.isError ? (
        <p className="text-sm text-danger" role="alert">
          {getGatewayApiKeyErrorMessage(query.error)}
        </p>
      ) : null}

      <GatewayApiKeyList
        configuration={configuration}
        pending={mutations.isPending || secretActions.pending}
        refreshing={query.isFetching}
        actionError={mutations.revoke.error}
        onCreate={() => openEditor("new")}
        onRefresh={() => void query.refetch()}
        onEdit={openEditor}
        onDelete={requestDelete}
      />

      <SideDrawer
        open={editorOpen}
        title={drawerTitle}
        description={drawerDescription}
        onClose={() => closeEditor(editorId)}
      >
        {editorInvalid ? (
          <div className="space-y-4 text-sm text-secondary">
            <p>可以从密钥列表重新进入。</p>
            <Button onClick={() => closeEditor(editorId)}>返回列表</Button>
          </div>
        ) : (
          <GatewayApiKeyEditor
            key={editorId}
            apiKey={selected}
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
        description="删除后该密钥会从列表和数据库中移除，旧 token 立即失效，不可恢复。"
        confirmLabel="确认删除"
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
