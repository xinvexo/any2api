import { ListChecks, Pencil, Trash2 } from "lucide-react";
import type { ReactNode } from "react";

import type { ProviderCredential } from "../api/provider-credential-contracts";
import type { ProxyConfiguration } from "@/features/proxies";
import { cn } from "@/shared/lib/cn";
import { RowActionButton } from "@/shared/ui/RowActionButton";
import { RequestUsageStats } from "@/shared/ui/RequestUsageStats";

export interface ProviderCredentialTableRowProps {
  credential: ProviderCredential;
  proxies: ProxyConfiguration;
  pending: boolean;
  embedded?: boolean;
  onEdit: (id: string) => void;
  onModels: (id: string) => void;
  onDelete: (credential: ProviderCredential) => void;
}

export function ProviderCredentialTableRow({
  credential,
  proxies,
  pending,
  embedded = false,
  onEdit,
  onModels,
  onDelete,
}: ProviderCredentialTableRowProps) {
  const proxyLabel = describeProxy(credential.proxyProfileId, proxies);
  const secretLabel = credential.secretTail
    ? `•••• ${credential.secretTail}`
    : credential.fingerprint;

  if (embedded) {
    return (
      <div
        data-floating-bounds
        className="min-w-0 max-w-full overflow-hidden rounded-[10px] bg-surface/80 px-2 py-2 sm:rounded-none sm:bg-transparent sm:px-0 sm:py-1.5"
      >
        <div className="flex min-w-0 max-w-full flex-col gap-2 sm:flex-row sm:items-center sm:gap-3">
          <div className="min-w-0 flex-1 space-y-1 overflow-hidden">
            <div className="flex min-w-0 flex-wrap items-baseline gap-x-1.5 gap-y-0.5">
              <span className="min-w-0 max-w-full truncate text-[12px] font-medium text-primary">
                {credential.label}
              </span>
              <span className="shrink-0 text-[10px] text-tertiary">
                {credential.models.length} 模型
              </span>
              <span
                className={cn(
                  "shrink-0 text-[10px]",
                  credential.enabled ? "text-success" : "text-tertiary",
                )}
              >
                {credential.enabled ? "启用" : "停用"}
              </span>
            </div>
            <div className="flex min-w-0 flex-wrap gap-x-2.5 gap-y-0.5 text-[11px] text-tertiary">
              <span className="min-w-0 max-w-full truncate" title={proxyLabel}>
                {proxyLabel}
              </span>
              <span className="shrink-0 tabular-nums">并发 {credential.maxConcurrency}</span>
              <span className="max-w-full truncate font-mono text-[10px]">{secretLabel}</span>
            </div>
          </div>

          <div className="w-full min-w-0 sm:w-[13rem] sm:shrink-0">
            <RequestUsageStats label={credential.label} usage={credential.usage} />
          </div>

          <div className="flex min-w-0 flex-wrap items-center justify-end gap-0.5 sm:shrink-0">
            <CredentialActions
              credential={credential}
              pending={pending}
              quiet
              onEdit={onEdit}
              onModels={onModels}
              onDelete={onDelete}
            />
          </div>
        </div>
      </div>
    );
  }

  return (
    <tr data-floating-bounds className="border-b border-subtle/50 last:border-b-0">
      <td className="py-2 pr-3 align-middle">
        <p className="flex min-w-0 flex-wrap items-baseline gap-x-1.5 break-words font-medium text-primary [overflow-wrap:anywhere]">
          <span className="min-w-0">{credential.label}</span>
          <span className="shrink-0 text-[11px] font-normal text-tertiary">
            {credential.models.length} 模型
          </span>
        </p>
      </td>
      <td className="px-3 py-2 align-middle">
        <span className="break-words [overflow-wrap:anywhere]">{proxyLabel}</span>
      </td>
      <td className="px-3 py-2 align-middle tabular-nums text-secondary">
        {credential.maxConcurrency}
      </td>
      <td className="px-3 py-2 align-middle">
        {credential.enabled ? (
          <Badge tone="success">已启用</Badge>
        ) : (
          <Badge>已停用</Badge>
        )}
      </td>
      <td className="px-3 py-2 align-middle">
        <span className="font-mono text-[11px] text-tertiary">{secretLabel}</span>
      </td>
      <td className="min-w-[10rem] px-3 py-2 align-middle">
        <RequestUsageStats label={credential.label} usage={credential.usage} />
      </td>
      <td className="py-2 pl-3 align-middle">
        <div className="flex flex-wrap items-center justify-end gap-0.5">
          <CredentialActions
            credential={credential}
            pending={pending}
            onEdit={onEdit}
            onModels={onModels}
            onDelete={onDelete}
          />
        </div>
      </td>
    </tr>
  );
}

function CredentialActions({
  credential,
  pending,
  quiet = false,
  onEdit,
  onModels,
  onDelete,
}: {
  credential: ProviderCredential;
  pending: boolean;
  quiet?: boolean;
  onEdit: (id: string) => void;
  onModels: (id: string) => void;
  onDelete: (credential: ProviderCredential) => void;
}) {
  return (
    <>
      <RowActionButton
        label={`配置 ${credential.label} 的模型`}
        disabled={pending}
        quiet={quiet}
        onClick={() => onModels(credential.id)}
      >
        <ListChecks size={quiet ? 12 : 13} />
        模型
      </RowActionButton>
      <RowActionButton
        label={`编辑 ${credential.label}`}
        disabled={pending}
        quiet={quiet}
        onClick={() => onEdit(credential.id)}
      >
        <Pencil size={quiet ? 12 : 13} />
        编辑
      </RowActionButton>
      <RowActionButton
        label={`删除 ${credential.label}`}
        disabled={pending}
        quiet={quiet}
        tone="danger"
        onClick={() => onDelete(credential)}
      >
        <Trash2 size={quiet ? 12 : 13} />
        删除
      </RowActionButton>
    </>
  );
}

function describeProxy(proxyId: string | undefined, configuration: ProxyConfiguration) {
  const proxy = configuration.items.find((item) => item.id === proxyId);
  if (!proxy) {
    return "出口代理配置不存在";
  }
  if (proxy.kind !== "direct") {
    return `${proxy.name}${proxy.enabled ? "" : " · 已停用"}`;
  }
  const global = configuration.items.find((item) => item.id === configuration.globalProxyId);
  return global?.kind === "direct"
    ? proxy.name
    : `${proxy.name} · 继承 ${global?.name ?? "未知出口代理"}`;
}

function Badge({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: "neutral" | "success";
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] font-medium",
        tone === "success" && "bg-success/10 text-success",
        tone === "neutral" && "bg-surface-muted text-secondary",
      )}
    >
      {children}
    </span>
  );
}
