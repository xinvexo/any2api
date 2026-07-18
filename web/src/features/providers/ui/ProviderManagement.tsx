import { Plus, RefreshCw, Server, TriangleAlert } from "lucide-react";
import { useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

import type { ProviderEndpoint, ProviderEndpointWriteInput } from "../api/provider-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderEndpointMutations } from "../model/use-provider-mutations";
import { useProviderEndpoints } from "../model/use-providers";
import { ProviderEndpointEditor } from "./ProviderEndpointEditor";
import { ProviderEndpointList } from "./ProviderEndpointList";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function ProviderManagement() {
  const endpoints = useProviderEndpoints();
  const mutations = useProviderEndpointMutations();
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const openedFromList = useRef(false);
  const editorId = searchParams.get("editor");
  const selected = editorId && editorId !== "new"
    ? endpoints.data?.items.find((endpoint) => endpoint.id === editorId)
    : undefined;

  function openEditor(id: string) {
    mutations.create.reset();
    mutations.update.reset();
    const fromList = editorId === null;
    if (fromList) {
      openedFromList.current = true;
    }
    setSearchParams((current) => {
      const next = new URLSearchParams(current);
      next.set("editor", id);
      return next;
    }, { replace: !fromList });
  }

  function closeEditor(expectedId: string | null = editorId) {
    mutations.create.reset();
    mutations.update.reset();
    if (expectedId && searchParams.get("editor") !== expectedId) {
      return;
    }
    if (openedFromList.current) {
      openedFromList.current = false;
      navigate(-1);
      return;
    }
    setSearchParams((current) => {
      const next = new URLSearchParams(current);
      next.delete("editor");
      return next;
    }, { replace: true });
  }

  if (endpoints.isPending && !endpoints.data) {
    return <Surface className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary" aria-busy="true">正在读取 Provider 配置</Surface>;
  }
  if (!endpoints.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取 Provider 配置</p>
        <p className="mt-2 text-sm text-secondary">{getProviderErrorMessage(endpoints.error)}</p>
        <Button className="mt-5" onClick={() => void endpoints.refetch()} disabled={endpoints.isFetching}>
          <RefreshCw size={15} className={endpoints.isFetching ? "animate-spin" : undefined} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = endpoints.data;
  const editorError = editorId === "new" ? mutations.create.error : mutations.update.error;
  const editorPending =
    endpoints.isFetching ||
    (editorId === "new" ? mutations.create.isPending : mutations.update.isPending);

  async function submitEditor(input: ProviderEndpointWriteInput) {
    if (editorId === "new") {
      await mutations.create.mutateAsync(input);
    } else if (editorId) {
      await mutations.update.mutateAsync({ id: editorId, input });
    }
  }

  function remove(id: string) {
    mutations.remove.reset();
    mutations.remove.mutate({ id, expectedRevision: configuration.configRevision });
  }

  return (
    <div className="space-y-5" aria-busy={mutations.isPending || endpoints.isFetching}>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-secondary">
          配置版本 <span className="font-medium tabular-nums text-primary">{configuration.configRevision}</span>
        </p>
        <div className="flex flex-col gap-2 sm:flex-row">
          <Button variant="ghost" onClick={() => void endpoints.refetch()} disabled={endpoints.isFetching}>
            <RefreshCw size={15} className={endpoints.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button variant="primary" onClick={() => openEditor("new")} disabled={mutations.isPending}>
            <Plus size={16} />
            新增 Endpoint
          </Button>
        </div>
      </div>

      {endpoints.isError ? (
        <Surface className="flex flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between" role="status">
          <p className="text-sm text-secondary">配置刷新失败，当前仍显示最近一次有效数据：{getProviderErrorMessage(endpoints.error)}</p>
          <Button onClick={() => void endpoints.refetch()} disabled={endpoints.isFetching}>重新加载</Button>
        </Surface>
      ) : null}

      <Surface className="flex gap-3 border-warning/40 bg-warning/5 p-4" role="note">
        <TriangleAlert size={18} className="mt-0.5 shrink-0 text-warning" aria-hidden="true" />
        <div className="text-sm leading-6 text-secondary">
          <p className="font-semibold text-primary">上游地址是服务端主动访问的目标</p>
          <p className="mt-1">默认只允许公网 HTTPS。HTTP、内网地址和本地服务必须按 Endpoint 单独显式开启；DNS 最终地址与重定向仍会在网络执行层再次检查。</p>
        </div>
      </Surface>

      <div className="grid gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(320px,0.72fr)] lg:items-start">
        <div className={editorId ? "order-2 lg:order-1" : undefined}>
          <ProviderEndpointList
            configuration={configuration}
            pending={mutations.isPending}
            actionError={mutations.remove.error}
            onEdit={openEditor}
            onDelete={remove}
          />
        </div>
        <div className={editorId ? "order-1 lg:order-2" : undefined}>
          {editorId ? (
            <ProviderEditorSlot
              key={editorId}
              editorId={editorId}
              currentEndpoint={selected}
              configRevision={configuration.configRevision}
              pending={editorPending}
              error={editorError}
              onSubmit={submitEditor}
              onClose={() => closeEditor(editorId)}
            />
          ) : (
            <EditorPlaceholder />
          )}
        </div>
      </div>
    </div>
  );
}

function ProviderEditorSlot({
  editorId,
  currentEndpoint,
  configRevision,
  pending,
  error,
  onSubmit,
  onClose,
}: {
  editorId: string;
  currentEndpoint?: ProviderEndpoint;
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSubmit: (input: ProviderEndpointWriteInput) => Promise<void>;
  onClose: () => void;
}) {
  const editing = editorId !== "new";
  const [initialEndpoint] = useState(currentEndpoint);
  if (editing && !initialEndpoint) {
    return <EditorPlaceholder invalid onClose={onClose} />;
  }
  const sourceConflict = editing
    ? !currentEndpoint
      ? "deleted"
      : currentEndpoint.configVersion !== initialEndpoint?.configVersion
        ? "changed"
        : null
    : null;

  return (
    <ProviderEndpointEditor
      endpoint={initialEndpoint}
      editing={editing}
      sourceConflict={sourceConflict}
      configRevision={configRevision}
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
        <Server size={22} className="mx-auto text-tertiary" aria-hidden="true" />
        <p className="mt-3 text-sm font-medium">
          {invalid ? "Provider Endpoint 不存在" : "选择一个 Endpoint 进行编辑"}
        </p>
        <p className="mt-1 text-sm text-secondary">
          {invalid ? "该链接可能已经过期，请返回列表。" : "也可以新增 Codex 或 Claude 上游地址。"}
        </p>
        {invalid && onClose ? <Button className="mt-4" onClick={onClose}>返回列表</Button> : null}
      </div>
    </Surface>
  );
}
