import { Plus, RefreshCw, Route } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";

import type { ProviderEndpoint } from "@/features/providers";
import { useProviderEndpoints } from "@/features/providers";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

import type { ModelRoute, ModelRouteWriteInput } from "../api/model-route-contracts";
import { getModelRouteErrorMessage } from "../model/model-route-error";
import { useModelRouteMutations } from "../model/use-model-route-mutations";
import { useModelRoutes } from "../model/use-model-routes";
import { ModelRouteEditor } from "./ModelRouteEditor";
import { ModelRouteList } from "./ModelRouteList";

export function ModelRouteManagement() {
  const routes = useModelRoutes();
  const endpoints = useProviderEndpoints();
  const mutations = useModelRouteMutations();
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const openedFromList = useRef(false);
  const editorId = searchParams.get("editor");
  const editorIdRef = useRef(editorId);
  useEffect(() => {
    editorIdRef.current = editorId;
  }, [editorId]);
  const selected =
    editorId && editorId !== "new"
      ? routes.data?.items.find((route) => route.id === editorId)
      : undefined;

  function openEditor(id: string) {
    mutations.create.reset();
    mutations.update.reset();
    const fromList = editorId === null;
    if (fromList) {
      openedFromList.current = true;
    }
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.set("editor", id);
        return next;
      },
      { replace: !fromList },
    );
  }

  function closeEditor(expectedId: string | null = editorIdRef.current) {
    mutations.create.reset();
    mutations.update.reset();
    if (expectedId && editorIdRef.current !== expectedId) {
      return;
    }
    if (openedFromList.current) {
      openedFromList.current = false;
      navigate(-1);
      return;
    }
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.delete("editor");
        return next;
      },
      { replace: true },
    );
  }

  if ((routes.isPending && !routes.data) || (endpoints.isPending && !endpoints.data)) {
    return (
      <Surface className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary" aria-busy="true">
        正在读取模型路由
      </Surface>
    );
  }
  if (!routes.data || !endpoints.data) {
    const message = !routes.data
      ? getModelRouteErrorMessage(routes.error)
      : "无法读取 Provider Endpoint";
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取模型路由配置</p>
        <p className="mt-2 text-sm text-secondary">{message}</p>
        <Button
          className="mt-5"
          onClick={() => void Promise.all([routes.refetch(), endpoints.refetch()])}
          disabled={routes.isFetching || endpoints.isFetching}
        >
          <RefreshCw size={15} />
          重试
        </Button>
      </Surface>
    );
  }

  const configuration = routes.data;
  const endpointItems = endpoints.data.items;
  const editorError = editorId === "new" ? mutations.create.error : mutations.update.error;
  const editorPending =
    routes.isFetching ||
    endpoints.isFetching ||
    (editorId === "new" ? mutations.create.isPending : mutations.update.isPending);

  async function submitEditor(input: ModelRouteWriteInput) {
    if (editorId === "new") {
      await mutations.create.mutateAsync(input);
    } else if (editorId) {
      await mutations.update.mutateAsync({ id: editorId, input });
    }
  }

  function remove(route: ModelRoute) {
    mutations.remove.reset();
    mutations.remove.mutate({
      id: route.id,
      expectedRevision: configuration.configRevision,
      expectedConfigVersion: route.configVersion,
    });
  }

  const refreshing = routes.isFetching || endpoints.isFetching;

  return (
    <div className="space-y-5" aria-busy={mutations.isPending || refreshing}>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-secondary">
          配置版本{" "}
          <span className="font-medium tabular-nums text-primary">
            {configuration.configRevision}
          </span>
        </p>
        <div className="flex flex-col gap-2 sm:flex-row">
          <Button
            variant="ghost"
            onClick={() => void Promise.all([routes.refetch(), endpoints.refetch()])}
            disabled={refreshing}
          >
            <RefreshCw size={15} className={refreshing ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button
            variant="primary"
            onClick={() => openEditor("new")}
            disabled={mutations.isPending}
          >
            <Plus size={16} />
            新增路由
          </Button>
        </div>
      </div>

      {routes.isError || endpoints.isError ? (
        <Surface className="flex flex-col gap-3 border-warning/40 p-4 sm:flex-row sm:items-center sm:justify-between" role="status">
          <p className="text-sm text-secondary">
            配置刷新失败，当前仍显示最近一次有效数据。
          </p>
          <Button
            onClick={() => void Promise.all([routes.refetch(), endpoints.refetch()])}
            disabled={refreshing}
          >
            重新加载
          </Button>
        </Surface>
      ) : null}

      <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_minmax(380px,0.82fr)] xl:items-start">
        <div className={editorId ? "hidden xl:block" : undefined}>
          <ModelRouteList
            configuration={configuration}
            endpoints={endpointItems}
            pending={mutations.isPending}
            actionError={mutations.remove.error}
            onEdit={openEditor}
            onDelete={remove}
          />
        </div>
        <div>
          {editorId ? (
            <ModelRouteEditorSlot
              key={
                editorId !== "new" && !selected && routes.isFetching
                  ? `${editorId}:loading`
                  : editorId
              }
              editorId={editorId}
              currentRoute={selected}
              endpoints={endpointItems}
              configRevision={configuration.configRevision}
              sourceLoading={routes.isFetching}
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

function ModelRouteEditorSlot({
  editorId,
  currentRoute,
  endpoints,
  configRevision,
  sourceLoading,
  pending,
  error,
  onSubmit,
  onClose,
}: {
  editorId: string;
  currentRoute?: ModelRoute;
  endpoints: ProviderEndpoint[];
  configRevision: number;
  sourceLoading: boolean;
  pending: boolean;
  error: unknown;
  onSubmit: (input: ModelRouteWriteInput) => Promise<void>;
  onClose: () => void;
}) {
  const editing = editorId !== "new";
  const [initialRoute] = useState(currentRoute);
  if (editing && !initialRoute) {
    return (
      <EditorPlaceholder
        invalid={!sourceLoading}
        loading={sourceLoading}
        onClose={onClose}
      />
    );
  }
  const sourceConflict = editing
    ? !currentRoute
      ? "deleted"
      : currentRoute.configVersion !== initialRoute?.configVersion
        ? "changed"
        : null
    : null;

  return (
    <ModelRouteEditor
      route={initialRoute}
      endpoints={endpoints}
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

function EditorPlaceholder({
  invalid = false,
  loading = false,
  onClose,
}: {
  invalid?: boolean;
  loading?: boolean;
  onClose?: () => void;
}) {
  return (
    <Surface
      className={`${invalid || loading ? "flex" : "hidden xl:flex"} min-h-52 items-center justify-center p-7 text-center xl:sticky xl:top-24`}
    >
      <div>
        <Route size={22} className="mx-auto text-tertiary" aria-hidden="true" />
        <p className="mt-3 text-sm font-medium">
          {loading
            ? "正在读取模型路由"
            : invalid
              ? "模型路由不存在"
              : "选择一个模型路由进行编辑"}
        </p>
        {!loading ? (
          <p className="mt-1 text-sm text-secondary">
            {invalid ? "该链接可能已经过期，请返回列表。" : "也可以新增一个公开模型映射。"}
          </p>
        ) : null}
        {invalid && onClose ? <Button className="mt-4" onClick={onClose}>返回列表</Button> : null}
      </div>
    </Surface>
  );
}
