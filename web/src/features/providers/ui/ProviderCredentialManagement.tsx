import { KeyRound, Plus, RefreshCw } from "lucide-react";
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
  const editorId = searchParams.get("credential");
  const action = searchParams.get("action");
  const mode = editorId === "new" ? "create" : action === "rotate" ? "rotate" : "edit";
  const selected = editorId && editorId !== "new"
    ? credentials.data?.items.find((credential) => credential.id === editorId)
    : undefined;

  function openEditor(id: string, nextAction?: "rotate") {
    mutations.update.reset();
    secretActions.reset();
    setSearchParams((current) => {
      const next = new URLSearchParams(current);
      next.set("credential", id);
      if (nextAction) {
        next.set("action", nextAction);
      } else {
        next.delete("action");
      }
      return next;
    });
  }

  function closeEditor() {
    setSearchParams((current) => {
      const next = new URLSearchParams(current);
      next.delete("credential");
      next.delete("action");
      return next;
    });
  }

  if ((credentials.isPending && !credentials.data) || (proxies.isPending && !proxies.data)) {
    return (
      <Surface className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary" aria-busy="true">
        正在读取 API Key 配置
      </Surface>
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
          <RefreshCw size={15} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = credentials.data;
  const pending =
    credentials.isFetching ||
    proxies.isFetching ||
    mutations.isPending ||
    secretActions.pending;
  const editorError = mode === "edit" ? mutations.update.error : secretActions.error;

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
      closeEditor();
    } catch {
      // Keep the local draft mounted after validation or version conflicts.
    }
  }

  function remove(credential: ProviderCredential) {
    mutations.remove.reset();
    mutations.remove.mutate({
      id: credential.id,
      expectedRevision: configuration.configRevision,
      expectedConfigVersion: credential.configVersion,
    });
  }

  return (
    <div className="space-y-5" aria-busy={pending}>
      {receipt ? (
        <CredentialSecretReceipt
          label={receipt.label}
          apiKey={receipt.apiKey}
          onClose={() => setReceipt(null)}
        />
      ) : null}

      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-secondary">
          配置版本 <span className="font-medium tabular-nums text-primary">{configuration.configRevision}</span>
        </p>
        <div className="flex flex-col gap-2 sm:flex-row">
          <Button
            variant="ghost"
            onClick={() => void Promise.all([credentials.refetch(), proxies.refetch()])}
            disabled={pending}
          >
            <RefreshCw size={15} className={credentials.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button variant="primary" onClick={() => openEditor("new")} disabled={pending}>
            <Plus size={16} />
            新增 API Key
          </Button>
        </div>
      </div>

      <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(320px,0.72fr)] lg:items-start">
        <div className={editorId ? "order-2 lg:order-1" : undefined}>
          <ProviderCredentialList
            configuration={configuration}
            proxies={proxies.data}
            pending={pending}
            actionError={mutations.remove.error}
            onEdit={(id) => openEditor(id)}
            onRotate={(id) => openEditor(id, "rotate")}
            onDelete={remove}
            endpoint={endpoint}
            testingCredentialId={credentialTest.testingCredentialId}
            testResults={credentialTest.results}
            testError={credentialTest.error}
            onTest={(id) => void credentialTest.test(id)}
          />
        </div>
        <div className={editorId ? "order-1 lg:order-2" : undefined}>
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
              onClose={closeEditor}
            />
          ) : (
            <EditorPlaceholder />
          )}
        </div>
      </div>
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
    return <EditorPlaceholder invalid onClose={onClose} />;
  }
  const sourceConflict = mode === "create"
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

function EditorPlaceholder({ invalid = false, onClose }: { invalid?: boolean; onClose?: () => void }) {
  return (
    <Surface className="flex min-h-52 items-center justify-center p-7 text-center lg:sticky lg:top-24">
      <div>
        <KeyRound size={22} className="mx-auto text-tertiary" aria-hidden="true" />
        <p className="mt-3 text-sm font-medium">
          {invalid ? "API Key 不存在" : "选择一个 API Key 进行管理"}
        </p>
        <p className="mt-1 text-sm text-secondary">
          {invalid ? "该链接可能已经过期。" : "也可以为这个 Endpoint 添加新的 Key。"}
        </p>
        {invalid && onClose ? <Button className="mt-4" onClick={onClose}>返回列表</Button> : null}
      </div>
    </Surface>
  );
}
