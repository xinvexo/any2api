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
  const [tokensById, setTokensById] = useState<Record<string, string>>({});
  const [deleteTarget, setDeleteTarget] = useState<GatewayApiKey | null>(null);
  const editorId = searchParams.get("editor");
  const selected =
    editorId && editorId !== "new"
      ? query.data?.items.find((key) => key.id === editorId)
      : undefined;
  const editorPending =
    mutations.update.isPending ||
    secretActions.pending;
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

  function rememberToken(id: string, token: string) {
    setTokensById((current) => ({ ...current, [id]: token }));
  }

  async function submitEditor(input: GatewayApiKeyEditorSubmit) {
    if (editorId === "new") {
      const result = await secretActions.create({
        expectedRevision: query.data?.configRevision ?? 0,
        name: input.name,
        enabled: input.enabled,
      });
      const created = [...result.configuration.items]
        .reverse()
        .find((item) => item.name === input.name && !item.revokedAt);
      if (created) {
        rememberToken(created.id, result.token);
      }
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
      const result = await secretActions.regenerate(current.id, {
        expectedRevision: revision,
        expectedConfigVersion: current.configVersion,
        expectedTokenVersion: current.tokenVersion,
      });
      rememberToken(current.id, result.token);
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
      setTokensById((current) => {
        if (!(deleteTarget.id in current)) {
          return current;
        }
        const next = { ...current };
        delete next[deleteTarget.id];
        return next;
      });
      setDeleteTarget(null);
    } catch {
      // Keep confirmation visible when the version is stale.
    }
  }

  if (query.isPending && !query.data) {
    return (
      <div className="flex min-h-56 items-center justify-center text-sm text-secondary" aria-busy="true">
        正在读取网关密钥
      </div>
    );
  }

  if (!query.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取网关密钥</p>
        <p className="mt-2 text-sm text-secondary">{getGatewayApiKeyErrorMessage(query.error)}</p>
        <Button className="mt-5" onClick={() => void query.refetch()} disabled={query.isFetching}>
          <RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = query.data;
  const editorInvalid = editorId !== null && editorId !== "new" && !selected;
  const editorOpen = editorId !== null;
  const editorError = secretActions.error ?? mutations.update.error;
  const drawerTitle = editorInvalid
    ? "密钥不存在"
    : editorId === "new"
      ? "新增密钥"
      : "编辑密钥";
  const drawerDescription = editorInvalid
    ? "该链接可能已经过期，请返回列表。"
    : "客户端使用这些密钥访问本地网关";

  return (
    <div aria-busy={query.isFetching || mutations.isPending || secretActions.pending}>
      {query.isError ? (
        <Surface
          className="mb-5 flex flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between"
          role="status"
        >
          <p className="text-sm text-secondary">
            配置刷新失败，当前仍显示最近一次有效数据：{getGatewayApiKeyErrorMessage(query.error)}
          </p>
          <Button onClick={() => void query.refetch()} disabled={query.isFetching}>
            重新加载
          </Button>
        </Surface>
      ) : null}

      <GatewayApiKeyList
        configuration={configuration}
        tokensById={tokensById}
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
        description="删除后该密钥立即失效，且不能重新启用。"
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
