import { RefreshCw } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { getGatewayApiKeyErrorMessage } from "../model/gateway-api-key-error";
import { useGatewayApiKeyMutations } from "../model/use-gateway-api-key-mutations";
import { useGatewayApiKeys } from "../model/use-gateway-api-keys";
import { notify } from "@/shared/notifications";
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
  const [searchParams, setSearchParams] = useSearchParams();
  const [rotateTarget, setRotateTarget] = useState<GatewayApiKey | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<GatewayApiKey | null>(null);
  const editorId = searchParams.get("editor");
  const selected =
    editorId && editorId !== "new"
      ? query.data?.items.find((key) => key.id === editorId)
      : undefined;
  const editorPending = mutations.create.isPending || mutations.update.isPending;
  const deletePending = mutations.remove.isPending;

  async function refreshKeys() {
    const result = await query.refetch();
    if (result.isSuccess) {
      notify.success("网关密钥已刷新");
    }
  }

  function openEditor(id: string) {
    setRotateTarget(null);
    setDeleteTarget(null);
    mutations.create.reset();
    mutations.update.reset();
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
      await mutations.create.mutateAsync({
        expectedRevision: query.data?.configRevision ?? 0,
        name: input.name,
        enabled: input.enabled,
      });
      notify.success(`已创建「${input.name}」`);
      closeEditor(editorId);
      return;
    }

    if (!selected || !query.data) {
      return;
    }

    const metaChanged =
      selected.name !== input.name || selected.enabled !== input.enabled;

    if (metaChanged) {
      await mutations.update.mutateAsync({
        id: selected.id,
        input: {
          expectedRevision: query.data.configRevision,
          expectedConfigVersion: selected.configVersion,
          name: input.name,
          enabled: input.enabled,
        },
      });
      notify.success(`已保存「${input.name}」`);
    } else {
      notify.info(`「${input.name}」没有需要保存的更改`);
    }

    closeEditor(editorId);
  }

  async function toggleEnabled(key: GatewayApiKey) {
    if (!query.data || mutations.update.isPending) {
      return;
    }
    const nextEnabled = !key.enabled;
    try {
      await mutations.update.mutateAsync({
        id: key.id,
        input: {
          expectedRevision: query.data.configRevision,
          expectedConfigVersion: key.configVersion,
          name: key.name,
          enabled: nextEnabled,
        },
      });
      notify.success(nextEnabled ? `已启用「${key.name}」` : `已禁用「${key.name}」`);
    } catch (error) {
      notify.danger(getGatewayApiKeyErrorMessage(error));
    }
  }

  function requestDelete(key: GatewayApiKey) {
    setRotateTarget(null);
    setDeleteTarget(key);
  }

  function requestRotate(key: GatewayApiKey) {
    setDeleteTarget(null);
    mutations.rotate.reset();
    setRotateTarget(key);
  }

  async function confirmRotate() {
    if (!rotateTarget || !query.data) {
      return;
    }
    try {
      await mutations.rotate.mutateAsync({
        id: rotateTarget.id,
        input: {
          expectedRevision: query.data.configRevision,
          expectedConfigVersion: rotateTarget.configVersion,
          expectedTokenVersion: rotateTarget.tokenVersion,
        },
      });
      notify.success(`已轮换「${rotateTarget.name}」的密钥`);
      setRotateTarget(null);
    } catch (error) {
      notify.danger(getGatewayApiKeyErrorMessage(error));
    }
  }

  async function confirmDelete() {
    if (!deleteTarget || !query.data) {
      return;
    }
    const target = deleteTarget;
    try {
      await mutations.remove.mutateAsync({
        id: target.id,
        input: {
          expectedRevision: query.data.configRevision,
          expectedConfigVersion: target.configVersion,
        },
      });
      notify.success(`已删除「${target.name}」`);
      setDeleteTarget(null);
    } catch (error) {
      notify.danger(getGatewayApiKeyErrorMessage(error));
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
        <Button onClick={() => void refreshKeys()}>
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
      ? "保存时由服务端生成强随机密钥，创建后可在列表复制。"
      : "这里只修改名称和启用状态；轮换密钥请使用列表中的独立操作。";
  const editorError = mutations.create.error ?? mutations.update.error;

  return (
    <div className="space-y-4">
      {query.isError ? (
        <p className="text-sm text-danger" role="alert">
          {getGatewayApiKeyErrorMessage(query.error)}
        </p>
      ) : null}

      <GatewayApiKeyList
        configuration={configuration}
        pending={mutations.isPending}
        refreshing={query.isFetching}
        actionError={mutations.remove.error ?? mutations.rotate.error}
        onCreate={() => openEditor("new")}
        onRefresh={() => void refreshKeys()}
        onEdit={openEditor}
        onToggleEnabled={(key) => void toggleEnabled(key)}
        onRotate={requestRotate}
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
        open={rotateTarget !== null}
        title={rotateTarget ? `轮换「${rotateTarget.name}」的密钥？` : ""}
        description="服务端会生成新的强随机 token，旧 token 在发布完成后立即失效。"
        confirmLabel="确认轮换"
        pending={mutations.rotate.isPending}
        onConfirm={() => void confirmRotate()}
        onClose={() => {
          if (!mutations.rotate.isPending) {
            setRotateTarget(null);
            mutations.rotate.reset();
          }
        }}
      />

      <ConfirmDialog
        open={deleteTarget !== null}
        title={deleteTarget ? `删除「${deleteTarget.name}」？` : ""}
        description="删除后该密钥会从列表和数据库中移除，旧 token 立即失效，不可恢复。"
        confirmLabel="确认删除"
        tone="danger"
        pending={deletePending}
        onConfirm={() => void confirmDelete()}
        onClose={() => {
          if (!deletePending) {
            setDeleteTarget(null);
          }
        }}
      />
    </div>
  );
}
