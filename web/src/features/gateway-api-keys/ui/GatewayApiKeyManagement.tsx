import { Copy, RefreshCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
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

interface RevealedGatewayApiKeySecret {
  kind: "create" | "rotate";
  name: string;
  token: string;
}

export function GatewayApiKeyManagement() {
  const query = useGatewayApiKeys();
  const mutations = useGatewayApiKeyMutations();
  const [searchParams, setSearchParams] = useSearchParams();
  const [rotateTarget, setRotateTarget] = useState<GatewayApiKey | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<GatewayApiKey | null>(null);
  const [revealedSecret, setRevealedSecret] =
    useState<RevealedGatewayApiKeySecret | null>(null);
  const editorId = searchParams.get("editor");
  const createdSecret = editorId === "new" && revealedSecret?.kind === "create"
    ? revealedSecret
    : null;
  const rotatedSecret = rotateTarget && revealedSecret?.kind === "rotate"
    ? revealedSecret
    : null;
  const createdSecretActionsRef = useRef<HTMLDivElement>(null);
  const selected =
    editorId && editorId !== "new"
      ? query.data?.items.find((key) => key.id === editorId)
      : undefined;
  const editorPending = mutations.create.isPending || mutations.update.isPending;
  const deletePending = mutations.remove.isPending;

  useEffect(() => {
    if (createdSecret) {
      createdSecretActionsRef.current
        ?.querySelector<HTMLButtonElement>("button")
        ?.focus({ preventScroll: true });
    }
  }, [createdSecret]);

  async function refreshKeys() {
    const result = await query.refetch();
    if (result.isSuccess) {
      notify.success("网关密钥已刷新");
    }
  }

  function openEditor(id: string) {
    setRevealedSecret(null);
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
    setRevealedSecret(null);
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
      const result = await mutations.create.mutateAsync({
        expectedRevision: input.expectedRevision,
        name: input.name,
        requestsPerMinute: input.requestsPerMinute,
        enabled: input.enabled,
      });
      setRevealedSecret({ kind: "create", name: input.name, token: result.token });
      mutations.create.reset();
      notify.success(`已创建「${input.name}」`);
      return;
    }

    if (!selected || input.expectedConfigVersion === null) {
      return;
    }

    const metaChanged =
      selected.name !== input.name ||
      selected.requestsPerMinute !== input.requestsPerMinute ||
      selected.enabled !== input.enabled;

    if (metaChanged) {
      await mutations.update.mutateAsync({
        id: selected.id,
        input: {
          expectedRevision: input.expectedRevision,
          expectedConfigVersion: input.expectedConfigVersion,
          name: input.name,
          requestsPerMinute: input.requestsPerMinute,
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
          requestsPerMinute: key.requestsPerMinute,
          enabled: nextEnabled,
        },
      });
      notify.success(nextEnabled ? `已启用「${key.name}」` : `已禁用「${key.name}」`);
    } catch (error) {
      notify.danger(getGatewayApiKeyErrorMessage(error));
    }
  }

  function requestDelete(key: GatewayApiKey) {
    setRevealedSecret(null);
    setRotateTarget(null);
    setDeleteTarget(key);
  }

  function requestRotate(key: GatewayApiKey) {
    setRevealedSecret(null);
    setDeleteTarget(null);
    mutations.rotate.reset();
    setRotateTarget(key);
  }

  async function confirmRotate() {
    if (!rotateTarget || !query.data) {
      return;
    }
    try {
      const result = await mutations.rotate.mutateAsync({
        id: rotateTarget.id,
        input: {
          expectedRevision: query.data.configRevision,
          expectedConfigVersion: rotateTarget.configVersion,
          expectedTokenVersion: rotateTarget.tokenVersion,
        },
      });
      setRevealedSecret({
        kind: "rotate",
        name: rotateTarget.name,
        token: result.token,
      });
      mutations.rotate.reset();
      notify.success(`已轮换「${rotateTarget.name}」的密钥`);
    } catch (error) {
      notify.danger(getGatewayApiKeyErrorMessage(error));
    }
  }

  async function copyRevealedSecret() {
    if (!revealedSecret) {
      return;
    }
    try {
      await navigator.clipboard.writeText(revealedSecret.token);
      notify.success(`已复制「${revealedSecret.name}」的密钥`);
    } catch {
      notify.danger("复制失败，请检查浏览器剪贴板权限后重试");
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
  const drawerTitle = createdSecret
    ? `保存「${createdSecret.name}」的新密钥`
    : editorId === "new"
      ? "新增"
      : selected
        ? `编辑「${selected.name}」`
        : "密钥不存在";
  const drawerDescription = createdSecret
    ? "关闭后无法再次查看；如未保存，请重新轮换密钥。"
    : editorId === "new"
      ? "保存时由服务端生成强随机密钥，创建成功后只显示一次。"
      : "这里可修改名称、RPM 限制和启用状态；轮换密钥请使用列表中的独立操作。";
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
        closeDisabled={editorPending}
        onClose={() => closeEditor(editorId)}
      >
        {createdSecret ? (
          <div className="space-y-5">
            <GatewayApiKeySecretValue token={createdSecret.token} />
            <div ref={createdSecretActionsRef} className="flex flex-wrap justify-end gap-2">
              <Button
                variant="secondary"
                onClick={() => void copyRevealedSecret()}
              >
                <Copy size={14} />
                复制密钥
              </Button>
              <Button onClick={() => closeEditor(editorId)}>已保存，关闭</Button>
            </div>
          </div>
        ) : editorInvalid ? (
          <div className="space-y-4 text-sm text-secondary">
            <p>可以从密钥列表重新进入。</p>
            <Button onClick={() => closeEditor(editorId)}>返回列表</Button>
          </div>
        ) : (
          <GatewayApiKeyEditor
            key={editorId}
            apiKey={selected}
            configRevision={configuration.configRevision}
            pending={editorPending}
            error={editorError}
            onSubmit={submitEditor}
            onClose={() => closeEditor(editorId)}
          />
        )}
      </SideDrawer>

      <ConfirmDialog
        key={rotatedSecret ? "rotate-secret" : "rotate-confirm"}
        open={rotateTarget !== null}
        title={
          rotatedSecret
            ? `保存「${rotatedSecret.name}」的新密钥`
            : rotateTarget
              ? `轮换「${rotateTarget.name}」的密钥？`
              : ""
        }
        description={
          rotatedSecret ? (
            <GatewayApiKeySecretValue token={rotatedSecret.token} />
          ) : (
            "服务端会生成新的强随机 token，旧 token 在发布完成后立即失效。"
          )
        }
        confirmLabel={rotatedSecret ? "复制密钥" : "确认轮换"}
        cancelLabel={rotatedSecret ? "已保存，关闭" : "取消"}
        pending={mutations.rotate.isPending}
        onConfirm={() =>
          void (rotatedSecret ? copyRevealedSecret() : confirmRotate())
        }
        onClose={() => {
          if (!mutations.rotate.isPending) {
            setRotateTarget(null);
            setRevealedSecret(null);
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

function GatewayApiKeySecretValue({ token }: { token: string }) {
  return (
    <div className="space-y-3">
      <p className="text-[13px] leading-5 text-warning" role="status">
        此密钥只显示一次。请立即复制并保存到安全位置。
      </p>
      <code className="block break-all rounded-[9px] bg-surface-muted p-3 font-mono text-[12px] leading-5 text-primary">
        {token}
      </code>
    </div>
  );
}
