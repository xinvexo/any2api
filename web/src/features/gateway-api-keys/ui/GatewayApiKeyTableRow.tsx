import { Ban, Copy, Pencil, Power, Trash2 } from "lucide-react";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { notify } from "@/shared/notifications";
import { cn } from "@/shared/lib/cn";
import { RequestUsageStats } from "@/shared/ui/RequestUsageStats";
import { RowActionButton } from "@/shared/ui/RowActionButton";

export interface GatewayApiKeyTableRowProps {
  apiKey: GatewayApiKey;
  pending: boolean;
  onEdit: (id: string) => void;
  onToggleEnabled: (key: GatewayApiKey) => void;
  onDelete: (key: GatewayApiKey) => void;
}

export function GatewayApiKeyTableRow({
  apiKey,
  pending,
  onEdit,
  onToggleEnabled,
  onDelete,
}: GatewayApiKeyTableRowProps) {
  async function copyToken() {
    try {
      await navigator.clipboard.writeText(apiKey.token);
      notify.success(`已复制「${apiKey.name}」的密钥`);
    } catch {
      notify.danger("复制失败，请检查浏览器剪贴板权限后重试");
    }
  }

  return (
    <tr className="border-b border-subtle last:border-b-0">
      <td className="py-2.5 pr-3 align-middle">
        <p className="break-words font-medium text-primary [overflow-wrap:anywhere]">{apiKey.name}</p>
      </td>
      <td className="px-3 py-2.5 align-middle">
        <RequestUsageStats label={apiKey.name} usage={apiKey.usage} />
      </td>
      <td className="px-3 py-2.5 align-middle">
        <Status enabled={apiKey.enabled} />
      </td>
      <td className="px-3 py-2.5 align-middle text-secondary tabular-nums">
        {apiKey.lastUsedAt ? formatTimestamp(apiKey.lastUsedAt) : "—"}
      </td>
      <td className="px-3 py-2.5 align-middle text-secondary tabular-nums">
        {formatTimestamp(apiKey.createdAt)}
      </td>
      <td className="py-2.5 pl-3 align-middle">
        <div className="flex flex-wrap items-center justify-end gap-0.5">
          <RowActionButton
            label={`复制 ${apiKey.name} 的密钥`}
            disabled={pending}
            onClick={() => void copyToken()}
          >
            <Copy size={13} />
            复制
          </RowActionButton>
          <RowActionButton
            label={apiKey.enabled ? `禁用 ${apiKey.name}` : `启用 ${apiKey.name}`}
            disabled={pending}
            onClick={() => onToggleEnabled(apiKey)}
          >
            {apiKey.enabled ? <Ban size={13} /> : <Power size={13} />}
            {apiKey.enabled ? "禁用" : "启用"}
          </RowActionButton>
          <RowActionButton
            label={`编辑 ${apiKey.name}`}
            disabled={pending}
            onClick={() => onEdit(apiKey.id)}
          >
            <Pencil size={13} />
            编辑
          </RowActionButton>
          <RowActionButton
            label={`删除 ${apiKey.name}`}
            disabled={pending}
            tone="danger"
            onClick={() => onDelete(apiKey)}
          >
            <Trash2 size={13} />
            删除
          </RowActionButton>
        </div>
      </td>
    </tr>
  );
}

function Status({ enabled }: { enabled: boolean }) {
  if (enabled) {
    return <Badge tone="success">已启用</Badge>;
  }
  return <Badge>已停用</Badge>;
}

function Badge({
  children,
  tone = "neutral",
}: {
  children: React.ReactNode;
  tone?: "neutral" | "success" | "danger";
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] font-medium",
        tone === "success" && "bg-success/10 text-success",
        tone === "danger" && "bg-danger/10 text-danger",
        tone === "neutral" && "bg-surface-muted text-secondary",
      )}
    >
      {children}
    </span>
  );
}

function formatTimestamp(value: string) {
  return value.replace(" ", " · ");
}
