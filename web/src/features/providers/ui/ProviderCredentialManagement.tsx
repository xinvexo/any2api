import { RefreshCw } from "lucide-react";
import { useState } from "react";
import { useSearchParams } from "react-router-dom";

import type { ProviderEndpoint } from "../api/provider-contracts";
import type { ProviderCredential } from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderCredentialMutations } from "../model/use-provider-credential-mutations";
import { useProviderCredentialTest } from "../model/use-provider-credential-test";
import { useProviderCredentials } from "../model/use-provider-credentials";
import { useProviderSecretActions } from "../model/use-provider-secret-actions";
import { CredentialSecretReceipt } from "./CredentialSecretReceipt";
import {
  ProviderCredentialEditor,
  type CredentialEditorSubmission,
} from "./ProviderCredentialEditor";
import { ProviderCredentialList } from "./ProviderCredentialList";
import { useCredentialProxyOptions } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { Surface } from "@/shared/ui/Surface";

export function ProviderCredentialManagement({ endpoint }: { endpoint: ProviderEndpoint }) {
  const credentials = useProviderCredentials(endpoint.id);
  const proxies = useCredentialProxyOptions();
  const mutations = useProviderCredentialMutations(endpoint.id);
  const secretActions = useProviderSecretActions(endpoint.id);
  const credentialTest = useProviderCredentialTest(
    `${credentials.data?.configRevision ?? 0}:${endpoint.configVersion}:${proxies.data?.configRevision ?? 0}`,
  );
  const [searchParams, setSearchParams] = useSearchParams();
  const [receipt, setReceipt] = useState<{ label: string; apiKey: string } | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<ProviderCredential | null>(null);
  const editorId = searchParams.get("credential");
  const action = searchParams.get("action");
  const mode = editorId === "new" ? "create" : action === "rotate" ? "rotate" : "edit";
  const selected =
    editorId && editorId !== "new"
      ? credentials.data?.items.find((credential) => credential.id === editorId)
      : undefined;

  function openEditor(id: string, nextAction?: "rotate") {
    mutations.update.reset();
    secretActions.reset();
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.set("credential", id);
        if (nextAction) {
          next.set("action", nextAction);
        } else {
          next.delete("action");
        }
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
        if (expectedId && current.get("credential") !== expectedId) {
          return current;
        }
        const next = new URLSearchParams(current);
        next.delete("credential");
        next.delete("action");
        return next;
      },
      { replace: true },
    );
  }

  if ((credentials.isPending && !credentials.data) || (proxies.isPending && !proxies.data)) {
    return (
      <div className="flex min-h-56 items-center justify-center text-sm text-secondary" aria-busy="true">
        正在读取 API Key 配置
      </div>
    );
  }

  if (!credentials.data || !proxies.data) {
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
  const pending =
    mutations.isPending ||
    secretActions.pending;
  const editorError = mode === "edit" ? mutations.update.error : secretActions.error;
  const editorOpen = editorId !== null;

  async function submit(submission: CredentialEditorSubmission) {
    try {
      if (submission.mode === "create") {
        await secretActions.create(submission.input);
        setReceipt({ label: submission.input.label, apiKey: submission.input.apiKey });
      } else if (submission.mode === "edit") {
        await mutations.update.mutateAsync({ id: submission.id, input: submission.input });
      } else {
        await secretActions.rotate(submission.id, submission.input);
        setReceipt({ label: submission.label, apiKey: submission.input.apiKey });
      }
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

  const drawerTitle =
    mode === "create" ? "新增 API Key" : mode === "rotate" ? "轮换 API Key" : "编辑 API Key";
  const drawerDescription =
    mode === "rotate" ? "写入新密钥并提升 secret 版本" : "绑定代理与并发限制";

  return (
    <div aria-busy={pending || credentials.isFetching || proxies.isFetching}>
      {receipt ? (
        <div className="mb-4">
          <CredentialSecretReceipt
            label={receipt.label}
            apiKey={receipt.apiKey}
            onClose={() => setReceipt(null)}
          />
        </div>
      ) : null}

      {(credentials.isError || proxies.isError) ? (
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

      <ProviderCredentialList
        configuration={configuration}
        proxies={proxies.data}
        pending={pending}
        refreshing={credentials.isFetching || proxies.isFetching}
        actionError={mutations.remove.error}
        endpoint={endpoint}
        testingCredentialId={credentialTest.testingCredentialId}
        testResults={credentialTest.results}
        testError={credentialTest.error}
        onCreate={() => openEditor("new")}
        onRefresh={() => void Promise.all([credentials.refetch(), proxies.refetch()])}
        onEdit={(id) => openEditor(id)}
        onRotate={(id) => openEditor(id, "rotate")}
        onDelete={setDeleteTarget}
        onTest={(id) => void credentialTest.test(id)}
      />

      <SideDrawer
        open={editorOpen}
        title={drawerTitle}
        description={drawerDescription}
        onClose={() => closeEditor(editorId)}
      >
        {editorId ? (
          <CredentialEditorSlot
            key={`${editorId}:${mode}`}
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

function CredentialEditorSlot({
  mode,
  currentCredential,
  configRevision,
  proxies,
  pending,
  error,
  onSubmit,
  onClose,
}: {
  mode: "create" | "edit" | "rotate";
  currentCredential?: ProviderCredential;
  configRevision: number;
  proxies: NonNullable<ReturnType<typeof useCredentialProxyOptions>["data"]>;
  pending: boolean;
  error: unknown;
  onSubmit: (submission: CredentialEditorSubmission) => Promise<void>;
  onClose: () => void;
}) {
  const [initialCredential] = useState(currentCredential);

  if (mode !== "create" && !initialCredential) {
    return (
      <div className="space-y-4 text-sm text-secondary">
        <p>API Key 不存在，该链接可能已经过期。</p>
        <Button onClick={onClose}>返回列表</Button>
      </div>
    );
  }

  const sourceConflict =
    mode === "create"
      ? null
      : !currentCredential
        ? "deleted"
        : currentCredential.configVersion !== initialCredential?.configVersion ||
            currentCredential.secretVersion !== initialCredential?.secretVersion
          ? "changed"
          : null;

  return (
    <ProviderCredentialEditor
      mode={mode}
      credential={initialCredential}
      sourceConflict={sourceConflict}
      configRevision={configRevision}
      proxies={proxies}
      pending={pending}
      error={error}
      onSubmit={onSubmit}
      onClose={onClose}
    />
  );
}
