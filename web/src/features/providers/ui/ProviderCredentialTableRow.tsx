import { ListChecks, Pencil, Trash2 } from "lucide-react";

import type { ProviderCredential } from "../api/provider-credential-contracts";
import type { ProxyConfiguration } from "@/features/proxies";
import { cn } from "@/shared/lib/cn";

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

  return (
    <tr
      className={cn(
        "border-b border-subtle last:border-b-0",
        embedded && "border-subtle/70",
      )}
    >
      <td className={cn("align-middle pr-3", embedded ? "py-1.5" : "py-2")}>
        <p
          className={cn(
            "flex min-w-0 flex-wrap items-baseline gap-x-1.5 break-words [overflow-wrap:anywhere]",
            embedded
              ? "font-normal text-secondary"
              : "font-medium text-primary",
          )}
        >
          <span className="min-w-0">{credential.label}</span>
          <span className="shrink-0 text-[11px] font-normal text-tertiary">
            {credential.models.length} 个模型
          </span>
        </p>
      </td>
      <td className={cn("px-3 align-middle", embedded ? "py-1.5 text-tertiary" : "py-2")}>
        <span className="break-words [overflow-wrap:anywhere]">{proxyLabel}</span>
      </td>
      <td
        className={cn(
          "px-3 align-middle tabular-nums",
          embedded ? "py-1.5 text-tertiary" : "py-2 text-secondary",
        )}
      >
        {credential.maxConcurrency}
      </td>
      <td className={cn("px-3 align-middle", embedded ? "py-1.5" : "py-2")}>
        {embedded ? (
          <span className="text-tertiary">
            {credential.enabled ? "已启用" : "已停用"}
          </span>
        ) : credential.enabled ? (
          <Badge tone="success">已启用</Badge>
        ) : (
          <Badge>已停用</Badge>
        )}
      </td>
      <td className={cn("px-3 align-middle", embedded ? "py-1.5" : "py-2")}>
        <span className="font-mono text-[11px] text-tertiary">{secretLabel}</span>
      </td>
      <td className={cn("pl-3 align-middle", embedded ? "py-1.5" : "py-2")}>
        <div className="flex flex-wrap items-center justify-end gap-0.5">
          <RowAction
            label={`配置 ${credential.label} 的模型`}
            disabled={pending}
            quiet={embedded}
            onClick={() => onModels(credential.id)}
          >
            <ListChecks size={embedded ? 12 : 13} />
            模型
          </RowAction>
          <RowAction
            label={`编辑 ${credential.label}`}
            disabled={pending}
            quiet={embedded}
            onClick={() => onEdit(credential.id)}
          >
            <Pencil size={embedded ? 12 : 13} />
            编辑
          </RowAction>
          <RowAction
            label={`删除 ${credential.label}`}
            disabled={pending}
            quiet={embedded}
            tone="danger"
            onClick={() => onDelete(credential)}
          >
            <Trash2 size={embedded ? 12 : 13} />
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
  quiet = false,
}: {
  label: string;
  children: React.ReactNode;
  disabled?: boolean;
  onClick: () => void;
  tone?: "accent" | "danger";
  quiet?: boolean;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "focus-ring inline-flex items-center gap-1 font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        quiet
          ? "h-6 rounded-[6px] px-1.5 text-[11px]"
          : "h-7 rounded-[7px] px-2 text-[12px]",
        tone === "danger"
          ? quiet
            ? "text-danger/70 hover:bg-danger/8 hover:text-danger"
            : "text-danger hover:bg-danger/8"
          : quiet
            ? "text-tertiary hover:bg-surface-muted hover:text-secondary"
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
