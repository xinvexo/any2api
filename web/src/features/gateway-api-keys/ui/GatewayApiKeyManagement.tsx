import { KeyRound, Plus, RefreshCw, ShieldOff, X } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { getGatewayApiKeyErrorMessage } from "../model/gateway-api-key-error";
import { useGatewayApiKeyMutations } from "../model/use-gateway-api-key-mutations";
import { useGatewayApiKeySecretActions } from "../model/use-gateway-api-key-secret-actions";
import { useGatewayApiKeys } from "../model/use-gateway-api-keys";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

import { GatewayApiKeyEditor } from "./GatewayApiKeyEditor";
import { GatewayApiKeyList } from "./GatewayApiKeyList";
import { GatewayApiKeySecretReceipt } from "./GatewayApiKeySecretReceipt";

type Confirmation = { action: "rotate" | "revoke"; key: GatewayApiKey };

export function GatewayApiKeyManagement() {
  const query = useGatewayApiKeys();
  const mutations = useGatewayApiKeyMutations();
  const secretActions = useGatewayApiKeySecretActions();
  const [searchParams, setSearchParams] = useSearchParams();
  const [receipt, setReceipt] = useState<{ name: string; token: string } | null>(null);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const editorId = searchParams.get("editor");
  const selected = editorId && editorId !== "new"
    ? query.data?.items.find((key) => key.id === editorId)
    : undefined;
  const pending = query.isFetching || mutations.isPending || secretActions.pending;

  function openEditor(id: string) {
    setReceipt(null);
    setConfirmation(null);
    mutations.update.reset();
    setSearchParams((current) => {
      const next = new URLSearchParams(current);
      next.set("editor", id);
      return next;
    }, { replace: true });
  }

  function closeEditor(expectedId: string | null = editorId) {
    setSearchParams((current) => {
      if (expectedId && current.get("editor") !== expectedId) {
        return current;
      }
      const next = new URLSearchParams(current);
      next.delete("editor");
      return next;
    }, { replace: true });
  }

  async function submitEditor(input: { name: string; enabled: boolean }) {
    try {
      if (editorId === "new") {
        const result = await secretActions.create({
          expectedRevision: query.data?.configRevision ?? 0,
          ...input,
        });
        setReceipt({ name: input.name, token: result.token });
      } else if (selected) {
        await mutations.update.mutateAsync({
          id: selected.id,
          input: {
            expectedRevision: query.data?.configRevision ?? 0,
            expectedConfigVersion: selected.configVersion,
            ...input,
          },
        });
      }
      closeEditor(editorId);
    } catch {
      // Keep the editor open so a conflict can be reviewed and retried.
    }
  }

  function requestRotate(key: GatewayApiKey) {
    setReceipt(null);
    setConfirmation({ action: "rotate", key });
  }

  function requestRevoke(key: GatewayApiKey) {
    setReceipt(null);
    setConfirmation({ action: "revoke", key });
  }

  async function confirmAction() {
    if (!confirmation || !query.data) {
      return;
    }
    const { action, key } = confirmation;
    try {
      if (action === "rotate") {
        const result = await secretActions.rotate(key.id, {
          expectedRevision: query.data.configRevision,
          expectedConfigVersion: key.configVersion,
          expectedTokenVersion: key.tokenVersion,
        });
        setReceipt({ name: key.name, token: result.token });
      } else {
        await mutations.revoke.mutateAsync({
          id: key.id,
          input: {
            expectedRevision: query.data.configRevision,
            expectedConfigVersion: key.configVersion,
          },
        });
      }
      setConfirmation(null);
    } catch {
      // Keep confirmation visible when the version is stale.
    }
  }

  if (query.isPending && !query.data) {
    return (
      <Surface className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary" aria-busy="true">
        正在读取网关密钥
      </Surface>
    );
  }
  if (!query.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取网关密钥</p>
        <p className="mt-2 text-sm text-secondary">{getGatewayApiKeyErrorMessage(query.error)}</p>
        <Button className="mt-5" onClick={() => void query.refetch()} disabled={query.isFetching}>
          <RefreshCw size={15} />
          重试
        </Button>
      </Surface>
    );
  }

  const editorInvalid = editorId !== null && editorId !== "new" && !selected;
  const editorError = editorId === "new" ? secretActions.error : mutations.update.error;

  return (
    <div className="space-y-5" aria-busy={pending}>
      {receipt ? (
        <GatewayApiKeySecretReceipt
          name={receipt.name}
          token={receipt.token}
          onClose={() => setReceipt(null)}
        />
      ) : null}

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-secondary">
          配置版本 <span className="font-medium tabular-nums text-primary">{query.data.configRevision}</span>
        </p>
        <div className="flex flex-col gap-2 sm:flex-row">
          <Button variant="ghost" onClick={() => void query.refetch()} disabled={pending}>
            <RefreshCw size={15} className={query.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button variant="primary" onClick={() => openEditor("new")} disabled={pending}>
            <Plus size={16} />
            新增密钥
          </Button>
        </div>
      </div>

      {query.isError ? (
        <Surface className="border-warning/40 p-4 text-sm text-secondary" role="status">
          配置刷新失败，当前仍显示最近一次有效数据：{getGatewayApiKeyErrorMessage(query.error)}
        </Surface>
      ) : null}

      {confirmation ? (
        <Surface className="flex flex-col gap-4 border-warning/40 p-5 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex min-w-0 items-start gap-3">
            <ShieldOff size={19} className="mt-0.5 shrink-0 text-warning" aria-hidden="true" />
            <p className="text-sm text-secondary">
              {confirmation.action === "rotate"
                ? `轮换“${confirmation.key.name}”后，旧密钥会立即失效。`
                : `撤销“${confirmation.key.name}”后，该密钥不能重新启用或轮换。`}
            </p>
          </div>
          <div className="flex shrink-0 gap-2">
            <Button onClick={() => setConfirmation(null)} disabled={pending}>
              <X size={15} />
              取消
            </Button>
            <Button variant={confirmation.action === "revoke" ? "danger" : "primary"} onClick={() => void confirmAction()} disabled={pending}>
              {confirmation.action === "rotate" ? "确认轮换" : "确认撤销"}
            </Button>
          </div>
        </Surface>
      ) : null}

      <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(320px,0.72fr)] lg:items-start">
        <div className={editorId ? "order-2 lg:order-1" : undefined}>
          <GatewayApiKeyList
            configuration={query.data}
            pending={pending}
            onEdit={openEditor}
            onRotate={requestRotate}
            onRevoke={requestRevoke}
          />
        </div>
        <div className={editorId ? "order-1 lg:order-2" : undefined}>
          {editorId && !editorInvalid ? (
            <GatewayApiKeyEditor
              key={editorId}
              apiKey={selected}
              configRevision={query.data.configRevision}
              pending={pending}
              error={editorError}
              onSubmit={submitEditor}
              onClose={() => closeEditor(editorId)}
            />
          ) : (
            <EditorPlaceholder invalid={editorInvalid} onClose={() => closeEditor(editorId)} />
          )}
        </div>
      </div>
    </div>
  );
}

function EditorPlaceholder({ invalid, onClose }: { invalid: boolean; onClose: () => void }) {
  return (
    <Surface className="flex min-h-52 items-center justify-center p-7 text-center lg:sticky lg:top-24">
      <div>
        <KeyRound size={22} className="mx-auto text-tertiary" aria-hidden="true" />
        <p className="mt-3 text-sm font-medium">{invalid ? "网关密钥不存在" : "选择一个密钥进行管理"}</p>
        <p className="mt-1 text-sm text-secondary">
          {invalid ? "该链接可能已经过期。" : "也可以从这里创建新的访问凭据。"}
        </p>
        {invalid ? <Button className="mt-4" onClick={onClose}>返回列表</Button> : null}
      </div>
    </Surface>
  );
}
