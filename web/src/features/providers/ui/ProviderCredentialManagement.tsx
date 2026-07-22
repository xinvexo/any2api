import { RefreshCw } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { ProviderEndpoint } from "../api/provider-contracts";
import type { ProviderCredential } from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderCredentialMutations } from "../model/use-provider-credential-mutations";
import { useProviderCredentials } from "../model/use-provider-credentials";
import { useProviderSecretActions } from "../model/use-provider-secret-actions";
import { CredentialSecretReceipt } from "./CredentialSecretReceipt";
import type { CredentialEditorSubmission } from "./ProviderCredentialEditor";
import { CredentialEditorSlot } from "./CredentialEditorSlot";
import { ProviderCredentialList } from "./ProviderCredentialList";
import { useCredentialProxyOptions } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { Surface } from "@/shared/ui/Surface";
import { cn } from "@/shared/lib/cn";

export function ProviderCredentialManagement({
  endpoint,
  embedded = false,
  showList = true,
  onRevealList,
}: {
  endpoint: ProviderEndpoint;
  embedded?: boolean;
  /** When false, only drawers/dialogs mount (no accordion body). */
  showList?: boolean;
  /** Expand parent accordion after mutations that should reveal the list. */
  onRevealList?: () => void;
}) {
  const credentials = useProviderCredentials(endpoint.id);
  const proxies = useCredentialProxyOptions();
  const mutations = useProviderCredentialMutations(endpoint.id);
  const secretActions = useProviderSecretActions(endpoint.id);
  const [searchParams, setSearchParams] = useSearchParams();
  const [receipt, setReceipt] = useState<{ label: string; apiKey: string } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ProviderCredential | null>(null);
  const activeEndpointId = searchParams.get("keys");
  const editorId = searchParams.get("credential");
  const isActiveEndpoint = activeEndpointId === endpoint.id;
  const mode = editorId === "new" ? "create" : "edit";
  const selected =
    isActiveEndpoint && editorId && editorId !== "new"
      ? credentials.data?.items.find((credential) => credential.id === editorId)
      : undefined;

  function openEditor(id: string) {
    mutations.update.reset();
    secretActions.reset();
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.delete("editor");
        next.delete("action");
        next.set("keys", endpoint.id);
        next.set("credential", id);
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
        if (current.get("keys") !== endpoint.id) {
          return current;
        }
        if (expectedId && current.get("credential") !== expectedId) {
          return current;
        }
        const next = new URLSearchParams(current);
        next.delete("keys");
        next.delete("credential");
        next.delete("action");
        return next;
      },
      { replace: true },
    );
  }

  if ((credentials.isPending && !credentials.data) || (proxies.isPending && !proxies.data)) {
    if (!showList) {
      return null;
    }
    return (
      <div
        className={cn(
          "flex items-center justify-center text-sm text-secondary",
          embedded ? "min-h-12 py-3" : "min-h-56",
        )}
        aria-busy="true"
      >
        正在读取 API Key 配置
      </div>
    );
  }

  if (!credentials.data || !proxies.data) {
    if (!showList) {
      return null;
    }
    if (embedded) {
      return (
        <div className="flex flex-wrap items-center justify-between gap-2 py-2" role="alert">
          <p className="text-[12px] text-danger">
            无法读取 API Key：{getProviderErrorMessage(credentials.error ?? proxies.error)}
          </p>
          <Button
            variant="ghost"
            onClick={() => void Promise.all([credentials.refetch(), proxies.refetch()])}
          >
            <RefreshCw size={14} />
            重试
          </Button>
        </div>
      );
    }
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取 API Key 配置</p>
        <p className="mt-2 text-sm text-secondary">
          {getProviderErrorMessage(credentials.error ?? proxies.error)}
        </p>
        <Button
          className="mt-5"
          onClick={() => void Promise.all([credentials.refetch(), proxies.refetch()])}
        >
          <RefreshCw size={14} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = credentials.data;
  const pending = mutations.isPending || secretActions.pending;
  const editorError =
    mode === "edit" ? (mutations.update.error ?? secretActions.error) : secretActions.error;
  const editorOpen = isActiveEndpoint && editorId !== null;

  async function submit(submission: CredentialEditorSubmission) {
    try {
      if (submission.mode === "create") {
        await secretActions.create(submission.input);
        setReceipt({ label: submission.input.label, apiKey: submission.input.apiKey });
      } else {
        const updated = await mutations.update.mutateAsync({
          id: submission.id,
          input: submission.input,
        });
        if (submission.apiKey) {
          const credential = updated.items.find((item) => item.id === submission.id);
          if (!credential) {
            throw new Error("credential missing after update");
          }
          await secretActions.rotate(submission.id, {
            expectedRevision: updated.configRevision,
            expectedConfigVersion: credential.configVersion,
            expectedSecretVersion: credential.secretVersion,
            apiKey: submission.apiKey,
          });
          setReceipt({ label: submission.input.label, apiKey: submission.apiKey });
        }
      }
      onRevealList?.();
      closeEditor(editorId);
    } catch {
      // Keep the local draft mounted after validation or version conflicts.
    }
  }

  function confirmDelete() {
    if (!deleteTarget) {
      return;
    }
    mutations.remove.reset();
    mutations.remove.mutate(
      {
        id: deleteTarget.id,
        expectedRevision: configuration.configRevision,
        expectedConfigVersion: deleteTarget.configVersion,
      },
      {
        onSettled: () => setDeleteTarget(null),
      },
    );
  }

  const drawerTitle = mode === "create" ? "新增 API Key" : "编辑 API Key";

  return (
    <div aria-busy={pending || credentials.isFetching || proxies.isFetching}>
      {showList && receipt ? (
        <div className={cn(embedded ? "pb-2 pt-2" : "mb-4")}>
          <CredentialSecretReceipt
            label={receipt.label}
            apiKey={receipt.apiKey}
            onClose={() => setReceipt(null)}
          />
        </div>
      ) : null}

      {showList && !embedded && (credentials.isError || proxies.isError) ? (
        <Surface
          className="mb-5 flex flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between"
          role="status"
        >
          <p className="text-sm text-secondary">
            配置刷新失败，当前仍显示最近一次有效数据：
            {getProviderErrorMessage(credentials.error ?? proxies.error)}
          </p>
          <Button
            onClick={() => void Promise.all([credentials.refetch(), proxies.refetch()])}
            disabled={credentials.isFetching || proxies.isFetching}
          >
            重新加载
          </Button>
        </Surface>
      ) : null}

      {showList ? (
        <ProviderCredentialList
          configuration={configuration}
          proxies={proxies.data}
          pending={pending}
          refreshing={credentials.isFetching || proxies.isFetching}
          actionError={mutations.remove.error}
          embedded={embedded}
          onCreate={() => openEditor("new")}
          onRefresh={() => void Promise.all([credentials.refetch(), proxies.refetch()])}
          onEdit={(id) => openEditor(id)}
          onDelete={setDeleteTarget}
        />
      ) : null}

      <SideDrawer
        open={editorOpen}
        title={drawerTitle}
        description="绑定代理与并发限制"
        onClose={() => closeEditor(editorId)}
      >
        {editorOpen && editorId ? (
          <CredentialEditorSlot
            key={`${endpoint.id}:${editorId}:${mode}`}
            mode={mode}
            currentCredential={selected}
            configRevision={configuration.configRevision}
            proxies={proxies.data}
            pending={pending}
            error={editorError}
            onSubmit={submit}
            onClose={() => closeEditor(editorId)}
          />
        ) : null}
      </SideDrawer>

      <ConfirmDialog
        open={deleteTarget !== null}
        title="删除 API Key"
        description={
          deleteTarget ? `确定删除「${deleteTarget.label}」？此操作不可恢复。` : undefined
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
