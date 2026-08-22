import { RefreshCw } from "lucide-react";
import { useState } from "react";

import type { ProviderEndpoint } from "../api/provider-contracts";
import type {
  CredentialModelSelection,
  ProviderCredential,
} from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderRouteState } from "../model/provider-route-state";
import { useProviderCredentialMutations } from "../model/use-provider-credential-mutations";
import { useProviderCredentials } from "../model/use-provider-credentials";
import { useProviderSecretActions } from "../model/use-provider-secret-actions";
import {
  providerCredentialTestScope,
  useProviderCredentialTest,
} from "../model/use-provider-credential-test";
import type { CredentialEditorSubmission } from "./ProviderCredentialEditor";
import { CredentialEditorSlot } from "./CredentialEditorSlot";
import { ProviderCredentialList } from "./ProviderCredentialList";
import { ProviderCredentialListSkeleton } from "./ProviderCredentialListSkeleton";
import { ProviderCredentialModels } from "./ProviderCredentialModels";
import { useProxyConfiguration } from "@/features/proxies";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { Surface } from "@/shared/ui/Surface";
import { useAccordionReveal } from "@/shared/ui/use-accordion-reveal";

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
  const proxies = useProxyConfiguration();
  const mutations = useProviderCredentialMutations(endpoint.id);
  const secretActions = useProviderSecretActions(endpoint.id);
  const routeState = useProviderRouteState();
  const [deleteTarget, setDeleteTarget] = useState<ProviderCredential | null>(null);
  const activeEndpointId = routeState.credentialEndpointId;
  const editorId = routeState.credentialId;
  const editorAction = routeState.credentialAction;
  const isActiveEndpoint = activeEndpointId === endpoint.id;
  const mode = editorId === "new" ? "create" : "edit";
  const selected =
    isActiveEndpoint && editorId && editorId !== "new"
      ? credentials.data?.items.find((credential) => credential.id === editorId)
      : undefined;
  const credentialTest = useProviderCredentialTest(
    providerCredentialTestScope(endpoint, selected, proxies.data),
  );
  const initialListPending =
    (credentials.isPending && !credentials.data) || (proxies.isPending && !proxies.data);
  const revealListContent = useAccordionReveal(showList, !initialListPending);

  async function refreshConfiguration() {
    const [credentialResult, proxyResult] = await Promise.all([
      credentials.refetch(),
      proxies.refetch(),
    ]);
    if (credentialResult.isSuccess && proxyResult.isSuccess) {
      notify.success(`「${endpoint.name}」的 API Key 配置已刷新`);
    }
  }

  function openEditor(id: string) {
    mutations.update.reset();
    secretActions.reset();
    routeState.openCredentialEditor(endpoint.id, id);
  }

  function openModels(id: string) {
    mutations.models.reset();
    routeState.openCredentialModels(endpoint.id, id);
  }

  function closeEditor(expectedId: string | null = editorId) {
    mutations.update.reset();
    secretActions.reset();
    routeState.closeCredential(endpoint.id, expectedId);
  }

  if (initialListPending || (showList && !revealListContent)) {
    if (!showList) {
      return null;
    }
    return embedded ? (
      <ProviderCredentialListSkeleton />
    ) : (
      <div className="min-h-56 px-3 py-4">
        <ProviderCredentialListSkeleton />
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
            onClick={() => void refreshConfiguration()}
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
          onClick={() => void refreshConfiguration()}
        >
          <RefreshCw size={14} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = credentials.data;
  const modelMode = editorAction === "models";
  const modelTesting = credentialTest.testingCredentialId === selected?.id;
  const pending = mutations.isPending || secretActions.pending || modelTesting;
  const editorError =
    modelMode
      ? (mutations.models.error ?? credentialTest.error)
      : mode === "edit"
        ? (mutations.update.error ?? secretActions.error)
        : secretActions.error;
  const editorOpen = isActiveEndpoint && editorId !== null;

  async function submit(submission: CredentialEditorSubmission) {
    try {
      if (submission.mode === "create") {
        const existingIds = new Set(configuration.items.map((credential) => credential.id));
        const createdConfiguration = await secretActions.create(submission.input);
        const created = createdConfiguration.items.find(
          (credential) =>
            !existingIds.has(credential.id)
            && credential.label === submission.input.label,
        ) ?? createdConfiguration.items.find(
          (credential) => !existingIds.has(credential.id),
        );
        if (!created) {
          throw new Error("credential missing after create");
        }
        notify.success(`已创建「${submission.input.label}」`);
        onRevealList?.();
        openModels(created.id);
      } else {
        let updated = await mutations.update.mutateAsync({
          id: submission.id,
          input: submission.input,
        });
        if (submission.apiKey) {
          const credential = updated.items.find((item) => item.id === submission.id);
          if (!credential) {
            throw new Error("credential missing after update");
          }
          updated = await secretActions.rotate(submission.id, {
            expectedRevision: updated.configRevision,
            expectedConfigVersion: credential.configVersion,
            expectedSecretVersion: credential.secretVersion,
            apiKey: submission.apiKey,
          });
          notify.success(`已更新「${submission.input.label}」的 API Key`);
          onRevealList?.();
          openModels(submission.id);
        } else {
          notify.success(`已保存「${submission.input.label}」`);
          onRevealList?.();
          closeEditor(submission.id);
        }
      }
    } catch {
      // Keep the local draft mounted after validation or version conflicts.
    }
  }

  async function saveModels(models: CredentialModelSelection[]) {
    if (!selected) {
      return;
    }
    try {
      await mutations.models.mutateAsync({
        id: selected.id,
        input: {
          expectedRevision: configuration.configRevision,
          expectedConfigVersion: selected.configVersion,
          models,
        },
      });
      notify.success(`已保存「${selected.label}」的模型选择`);
      onRevealList?.();
      closeEditor(selected.id);
    } catch {
      // The mutation exposes the inline error while the local selection stays mounted.
    }
  }

  async function discoverModels(manual = false) {
    if (!selected) {
      return;
    }
    const result = await credentialTest.test(selected.id);
    if (manual && result?.accepted && result.catalogValid) {
      notify.success(`已读取「${selected.label}」的上游模型`);
    }
  }

  function toggleCredential(credential: ProviderCredential) {
    const nextEnabled = !credential.enabled;
    mutations.update.reset();
    mutations.update.mutate(
      {
        id: credential.id,
        input: {
          expectedRevision: configuration.configRevision,
          expectedConfigVersion: credential.configVersion,
          label: credential.label,
          proxyProfileId: credential.proxyProfileId,
          requestsPerMinute: credential.requestsPerMinute,
          enabled: nextEnabled,
        },
      },
      {
        onSuccess: () => {
          notify.success(
            nextEnabled ? `已启用「${credential.label}」` : `已停用「${credential.label}」`,
          );
        },
        onError: (error) => {
          notify.danger(getProviderErrorMessage(error));
        },
      },
    );
  }

  function confirmDelete() {
    if (!deleteTarget) {
      return;
    }
    const target = deleteTarget;
    mutations.remove.reset();
    mutations.remove.mutate(
      {
        id: target.id,
        expectedRevision: configuration.configRevision,
        expectedConfigVersion: target.configVersion,
      },
      {
        onSuccess: () => {
          notify.success(`已删除「${target.label}」`);
          setDeleteTarget(null);
        },
        onError: (error) => {
          notify.danger(getProviderErrorMessage(error));
          setDeleteTarget(null);
        },
      },
    );
  }

  const drawerTitle = mode === "create" ? "新增" : "编辑 API Key";

  return (
    <div aria-busy={pending || credentials.isFetching || proxies.isFetching}>
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
            onClick={() => void refreshConfiguration()}
            disabled={credentials.isFetching || proxies.isFetching}
          >
            重新加载
          </Button>
        </Surface>
      ) : null}

      {showList ? (
        <div className={embedded ? "min-h-[109px] sm:min-h-[51px]" : undefined}>
          <ProviderCredentialList
            configuration={configuration}
            pending={pending}
            refreshing={credentials.isFetching || proxies.isFetching}
            embedded={embedded}
            onCreate={() => openEditor("new")}
            onRefresh={() => void refreshConfiguration()}
            onEdit={(id) => openEditor(id)}
            onModels={openModels}
            onToggleEnabled={toggleCredential}
            onDelete={setDeleteTarget}
          />
        </div>
      ) : null}

      <SideDrawer
        open={editorOpen}
        title={modelMode ? "选择上游模型" : drawerTitle}
        description={
          modelMode ? "发现或手动配置这把 API Key 可用的模型" : "绑定出口代理与可选 RPM 限制"
        }
        onClose={() => closeEditor(editorId)}
      >
        {editorOpen && editorId && modelMode && selected ? (
          <ProviderCredentialModels
            key={`${selected.id}:${selected.configVersion}`}
            credential={selected}
            result={credentialTest.results[selected.id]}
            discovering={modelTesting}
            saving={mutations.models.isPending}
            error={editorError}
            onDiscover={(manual) => void discoverModels(manual)}
            onSave={saveModels}
            onClose={() => closeEditor(editorId)}
          />
        ) : editorOpen && editorId ? (
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
