import { ListChecks, Pencil, Trash2 } from "lucide-react";

import type { ProviderCredential } from "../api/provider-credential-contracts";
import type { ProxyConfiguration } from "@/features/proxies";
import { cn } from "@/shared/lib/cn";

export interface ProviderCredentialTableRowProps {
  credential: ProviderCredential;
  proxies: ProxyConfiguration;
  pending: boolean;
  onEdit: (id: string) => void;
  onModels: (id: string) => void;
  onDelete: (credential: ProviderCredential) => void;
}

export function ProviderCredentialTableRow({
  credential,
  proxies,
  pending,
  onEdit,
  onModels,
  onDelete,
}: ProviderCredentialTableRowProps) {
  const proxyLabel = describeProxy(credential.proxyProfileId, proxies);

  return (
    <tr className="border-b border-subtle last:border-b-0">
      <td className="py-2 pr-3 align-middle">
        <p className="break-words font-medium text-primary [overflow-wrap:anywhere]">
          {credential.label}
        </p>
        <p className="mt-0.5 text-[11px] text-tertiary">
          {credential.models.length} 个模型
        </p>
      </td>
      <td className="px-3 py-2 align-middle">
        <span className="break-words text-secondary [overflow-wrap:anywhere]">{proxyLabel}</span>
      </td>
      <td className="px-3 py-2 align-middle tabular-nums text-secondary">
        {credential.maxConcurrency}
      </td>
      <td className="px-3 py-2 align-middle">
        {credential.enabled ? <Badge tone="success">已启用</Badge> : <Badge>已停用</Badge>}
      </td>
      <td className="px-3 py-2 align-middle">
        <span className="font-mono text-[11px] text-tertiary">
          {credential.secretTail ? `•••• ${credential.secretTail}` : credential.fingerprint}
        </span>
      </td>
      <td className="py-2 pl-3 align-middle">
        <div className="flex flex-wrap items-center justify-end gap-0.5">
          <RowAction
            label={`配置 ${credential.label} 的模型`}
            disabled={pending}
            onClick={() => onModels(credential.id)}
          >
            <ListChecks size={13} />
            模型
          </RowAction>
          <RowAction
            label={`编辑 ${credential.label}`}
            disabled={pending}
            onClick={() => onEdit(credential.id)}
          >
            <Pencil size={13} />
            编辑
          </RowAction>
          <RowAction
            label={`删除 ${credential.label}`}
            disabled={pending}
            tone="danger"
            onClick={() => onDelete(credential)}
          >
            <Trash2 size={13} />
            删除
          </RowAction>
        </div>
      </td>
    </tr>
  );
}

function describeProxy(proxyId: string | undefined, configuration: ProxyConfiguration) {
  const proxy = configuration.items.find((item) => item.id === proxyId);
  if (!proxy) {
    return "代理配置不存在";
  }
  if (proxy.kind !== "direct") {
    return `${proxy.name}${proxy.enabled ? "" : " · 已停用"}`;
  }
  const global = configuration.items.find((item) => item.id === configuration.globalProxyId);
  return global?.kind === "direct"
    ? proxy.name
    : `${proxy.name} · 继承 ${global?.name ?? "未知代理"}`;
}

function RowAction({
  label,
  children,
  disabled,
  onClick,
  tone = "accent",
}: {
  label: string;
  children: React.ReactNode;
  disabled?: boolean;
  onClick: () => void;
  tone?: "accent" | "danger";
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "focus-ring inline-flex h-7 items-center gap-1 rounded-[7px] px-2 text-[12px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        tone === "danger"
          ? "text-danger hover:bg-danger/8"
          : "text-secondary hover:bg-surface-muted hover:text-primary",
      )}
    >
      {children}
    </button>
  );
}

function Badge({
  children,
  tone = "neutral",
}: {
  children: React.ReactNode;
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
