import { ListChecks, Pencil, Trash2 } from "lucide-react";

import type { ProviderCredential } from "../api/provider-credential-contracts";
import type { ProxyConfiguration } from "@/features/proxies";
import { cn } from "@/shared/lib/cn";
import { RowActionButton } from "@/shared/ui/RowActionButton";

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
            API Key · {credential.models.length} 个模型
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
          <RowActionButton
            label={`配置 ${credential.label} 的模型`}
            disabled={pending}
            quiet={embedded}
            onClick={() => onModels(credential.id)}
          >
            <ListChecks size={embedded ? 12 : 13} />
            模型
          </RowActionButton>
          <RowActionButton
            label={`编辑 ${credential.label}`}
            disabled={pending}
            quiet={embedded}
            onClick={() => onEdit(credential.id)}
          >
            <Pencil size={embedded ? 12 : 13} />
            编辑
          </RowActionButton>
          <RowActionButton
            label={`删除 ${credential.label}`}
            disabled={pending}
            quiet={embedded}
            tone="danger"
            onClick={() => onDelete(credential)}
          >
            <Trash2 size={embedded ? 12 : 13} />
            删除
          </RowActionButton>
        </div>
      </td>
    </tr>
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
