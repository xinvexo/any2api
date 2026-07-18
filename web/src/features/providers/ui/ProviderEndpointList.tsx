import { Check, Code2, Pencil, ShieldAlert, Trash2, X } from "lucide-react";
import { useState } from "react";

import type { ProviderEndpoint, ProviderEndpointConfiguration } from "../api/provider-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

interface ProviderEndpointListProps {
  configuration: ProviderEndpointConfiguration;
  pending: boolean;
  actionError: unknown;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
}

export function ProviderEndpointList({
  configuration,
  pending,
  actionError,
  onEdit,
  onDelete,
}: ProviderEndpointListProps) {
  return (
    <Surface className="overflow-hidden">
      <div className="flex items-center justify-between border-b border-subtle px-5 py-4 sm:px-6">
        <div>
          <h2 className="text-base font-semibold">Provider Endpoint</h2>
          <p className="mt-1 text-sm text-secondary">一个 URL 可以在后续绑定多个独立 Credential</p>
        </div>
        <span className="text-sm tabular-nums text-tertiary">{configuration.items.length}</span>
      </div>
      {configuration.items.length > 0 ? (
        <ul className="divide-y divide-subtle">
          {configuration.items.map((endpoint) => (
            <EndpointRow
              key={endpoint.id}
              endpoint={endpoint}
              pending={pending}
              onEdit={onEdit}
              onDelete={onDelete}
            />
          ))}
        </ul>
      ) : (
        <div className="p-7 text-center">
          <Code2 size={23} className="mx-auto text-tertiary" aria-hidden="true" />
          <p className="mt-3 text-sm font-medium">还没有 Provider Endpoint</p>
          <p className="mt-1 text-sm text-secondary">先添加上游地址，Credential 会在下一步单独配置。</p>
        </div>
      )}
      {actionError ? (
        <p className="border-t border-subtle px-5 py-3 text-sm text-danger sm:px-6" role="alert">
          {getProviderErrorMessage(actionError)}
        </p>
      ) : null}
    </Surface>
  );
}

function EndpointRow({
  endpoint,
  pending,
  onEdit,
  onDelete,
}: {
  endpoint: ProviderEndpoint;
  pending: boolean;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const hasRisk = endpoint.allowInsecureHttp || endpoint.allowPrivateNetwork;

  return (
    <li className="px-5 py-5 sm:px-6">
      <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <p className="break-words font-semibold [overflow-wrap:anywhere]">{endpoint.name}</p>
            <Badge>{endpoint.providerKind.toUpperCase()}</Badge>
            <Badge>{endpoint.protocolDialect === "openai_responses" ? "RESPONSES" : "MESSAGES"}</Badge>
            {endpoint.enabled ? (
              <Badge icon={<Check size={12} />}>启用</Badge>
            ) : (
              <Badge icon={<X size={12} />}>已停用</Badge>
            )}
            {hasRisk ? <Badge icon={<ShieldAlert size={12} />}>显式网络授权</Badge> : null}
          </div>
          <p className="mt-2 break-all text-sm text-secondary">{endpoint.baseUrl}</p>
          <p className="mt-1 text-xs text-tertiary">
            {endpoint.allowInsecureHttp ? "允许 HTTP" : "HTTPS 优先"} · {endpoint.allowPrivateNetwork ? "允许内网地址" : "公网地址"}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button variant="ghost" disabled={pending} aria-label={`编辑 ${endpoint.name}`} onClick={() => onEdit(endpoint.id)}>
            <Pencil size={15} />
            编辑
          </Button>
          {confirmDelete ? (
            <>
              <Button variant="danger" disabled={pending} aria-label={`确认删除 ${endpoint.name}`} onClick={() => onDelete(endpoint.id)}>
                <Trash2 size={15} />
                确认删除
              </Button>
              <Button variant="ghost" disabled={pending} aria-label={`取消删除 ${endpoint.name}`} onClick={() => setConfirmDelete(false)}>
                <X size={15} />
                取消
              </Button>
            </>
          ) : (
            <Button variant="ghost" disabled={pending} aria-label={`删除 ${endpoint.name}`} onClick={() => setConfirmDelete(true)}>
              <Trash2 size={15} />
              删除
            </Button>
          )}
        </div>
      </div>
    </li>
  );
}

function Badge({ children, icon }: { children: React.ReactNode; icon?: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-surface-muted px-2 py-1 text-[11px] font-semibold text-secondary">
      {icon}
      {children}
    </span>
  );
}
