import { Check, Pencil, Route, Trash2, X } from "lucide-react";
import { useState } from "react";

import type { ProviderEndpoint } from "@/features/providers";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

import type {
  ModelRoute,
  ModelRouteConfiguration,
} from "../api/model-route-contracts";
import { getModelRouteErrorMessage } from "../model/model-route-error";

interface ModelRouteListProps {
  configuration: ModelRouteConfiguration;
  endpoints: ProviderEndpoint[];
  pending: boolean;
  actionError: unknown;
  onEdit: (id: string) => void;
  onDelete: (route: ModelRoute) => void;
}

export function ModelRouteList({
  configuration,
  endpoints,
  pending,
  actionError,
  onEdit,
  onDelete,
}: ModelRouteListProps) {
  return (
    <Surface className="overflow-hidden">
      <div className="flex items-center justify-between border-b border-subtle px-5 py-4 sm:px-6">
        <div>
          <h2 className="text-base font-semibold">已发布路由</h2>
          <p className="mt-1 text-sm text-secondary">公开模型与上游目标</p>
        </div>
        <span className="text-sm tabular-nums text-tertiary">{configuration.items.length}</span>
      </div>
      {configuration.items.length > 0 ? (
        <ul className="divide-y divide-subtle">
          {configuration.items.map((route) => (
            <RouteRow
              key={route.id}
              route={route}
              endpoints={endpoints}
              pending={pending}
              onEdit={onEdit}
              onDelete={onDelete}
            />
          ))}
        </ul>
      ) : (
        <div className="p-7 text-center">
          <Route size={23} className="mx-auto text-tertiary" aria-hidden="true" />
          <p className="mt-3 text-sm font-medium">还没有模型路由</p>
          <p className="mt-1 text-sm text-secondary">新增路由后，公开模型才能映射到上游。</p>
        </div>
      )}
      {actionError ? (
        <p className="border-t border-subtle px-5 py-3 text-sm text-danger sm:px-6" role="alert">
          {getModelRouteErrorMessage(actionError)}
        </p>
      ) : null}
    </Surface>
  );
}

function RouteRow({
  route,
  endpoints,
  pending,
  onEdit,
  onDelete,
}: {
  route: ModelRoute;
  endpoints: ProviderEndpoint[];
  pending: boolean;
  onEdit: (id: string) => void;
  onDelete: (route: ModelRoute) => void;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const endpointNames = new Map(endpoints.map((endpoint) => [endpoint.id, endpoint.name]));
  const tierCount = new Set(route.targets.map((target) => target.fallbackTier)).size;

  return (
    <li className="px-5 py-5 sm:px-6">
      <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <p className="break-words font-semibold [overflow-wrap:anywhere]">{route.publicModel}</p>
            <Badge>
              {route.ingressProtocol === "openai_responses" ? "RESPONSES" : "MESSAGES"}
            </Badge>
            {route.enabled ? (
              <Badge icon={<Check size={12} />}>启用</Badge>
            ) : (
              <Badge icon={<X size={12} />}>已停用</Badge>
            )}
          </div>
          <p className="mt-2 text-sm text-secondary">
            {route.targets.length} 个 Target · {tierCount} 个 tier · {fallbackLabel(route.fallbackOnSaturation)}
          </p>
          <p className="mt-1 break-words text-xs text-tertiary [overflow-wrap:anywhere]">
            {route.targets
              .map((target) => endpointNames.get(target.providerEndpointId) ?? target.providerEndpointId)
              .filter((name, index, values) => values.indexOf(name) === index)
              .join(" · ")}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="ghost"
            disabled={pending}
            aria-label={`编辑 ${route.publicModel}`}
            onClick={() => onEdit(route.id)}
          >
            <Pencil size={15} />
            编辑
          </Button>
          {confirmDelete ? (
            <>
              <Button
                variant="danger"
                disabled={pending}
                aria-label={`确认删除 ${route.publicModel}`}
                onClick={() => onDelete(route)}
              >
                <Trash2 size={15} />
                确认删除
              </Button>
              <Button
                variant="ghost"
                disabled={pending}
                aria-label={`取消删除 ${route.publicModel}`}
                onClick={() => setConfirmDelete(false)}
              >
                <X size={15} />
                取消
              </Button>
            </>
          ) : (
            <Button
              variant="ghost"
              disabled={pending}
              aria-label={`删除 ${route.publicModel}`}
              onClick={() => setConfirmDelete(true)}
            >
              <Trash2 size={15} />
              删除
            </Button>
          )}
        </div>
      </div>
    </li>
  );
}

function fallbackLabel(value: boolean | null) {
  if (value === true) {
    return "满载进入下一 tier";
  }
  if (value === false) {
    return "满载停留当前 tier";
  }
  return "满载策略继承全局";
}

function Badge({ children, icon }: { children: React.ReactNode; icon?: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-surface-muted px-2 py-1 text-[11px] font-semibold text-secondary">
      {icon}
      {children}
    </span>
  );
}
