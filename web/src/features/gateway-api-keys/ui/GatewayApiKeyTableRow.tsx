import { Check, Copy, Eye, EyeOff, Pencil, Trash2 } from "lucide-react";
import { useState } from "react";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { cn } from "@/shared/lib/cn";

export interface GatewayApiKeyTableRowProps {
  apiKey: GatewayApiKey;
  pending: boolean;
  onEdit: (id: string) => void;
  onDelete: (key: GatewayApiKey) => void;
}

export function GatewayApiKeyTableRow({
  apiKey,
  pending,
  onEdit,
  onDelete,
}: GatewayApiKeyTableRowProps) {
  const [revealed, setRevealed] = useState(true);
  const [copied, setCopied] = useState(false);

  async function copyToken() {
    try {
      await navigator.clipboard.writeText(apiKey.token);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      setRevealed(true);
    }
  }

  return (
    <tr className="border-b border-subtle last:border-b-0">
      <td className="py-2.5 pr-3 align-middle">
        <p className="break-words font-medium text-primary [overflow-wrap:anywhere]">{apiKey.name}</p>
      </td>
      <td className="px-3 py-2.5 align-middle">
        <div className="flex min-w-0 items-center gap-1">
          <code className="max-w-[22rem] truncate font-mono text-[12px] text-primary">
            {revealed ? apiKey.token : maskToken(apiKey.token)}
          </code>
          <button
            type="button"
            className="focus-ring inline-flex size-7 shrink-0 items-center justify-center rounded-[7px] text-secondary hover:bg-surface-muted hover:text-primary"
            aria-label={revealed ? `隐藏 ${apiKey.name} 的密钥` : `显示 ${apiKey.name} 的密钥`}
            onClick={() => setRevealed((value) => !value)}
          >
            {revealed ? <EyeOff size={13} /> : <Eye size={13} />}
          </button>
          <button
            type="button"
            className="focus-ring inline-flex size-7 shrink-0 items-center justify-center rounded-[7px] text-secondary hover:bg-surface-muted hover:text-primary"
            aria-label={`复制 ${apiKey.name} 的密钥`}
            onClick={() => void copyToken()}
          >
            {copied ? <Check size={13} /> : <Copy size={13} />}
          </button>
        </div>
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
          <RowAction
            label={`编辑 ${apiKey.name}`}
            disabled={pending}
            onClick={() => onEdit(apiKey.id)}
          >
            <Pencil size={13} />
            编辑
          </RowAction>
          <RowAction
            label={`删除 ${apiKey.name}`}
            disabled={pending}
            tone="danger"
            onClick={() => onDelete(apiKey)}
          >
            <Trash2 size={13} />
            删除
          </RowAction>
        </div>
      </td>
    </tr>
  );
}

function maskToken(token: string) {
  if (token.length <= 12) {
    return "••••••••";
  }
  return `${token.slice(0, 8)}${"•".repeat(12)}${token.slice(-4)}`;
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

function formatTimestamp(value: string) {
  return value.replace(" ", " · ");
}
