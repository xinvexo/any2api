import { Check, Copy, Eye, EyeOff, Pencil, Trash2 } from "lucide-react";
import { useState } from "react";

import type { GatewayApiKey, GatewayApiKeyUsage } from "../api/gateway-api-key-contracts";
import { cn } from "@/shared/lib/cn";
import { IconButton } from "@/shared/ui/IconButton";
import { RowActionButton } from "@/shared/ui/RowActionButton";

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
          <IconButton
            size="sm"
            className="size-7 rounded-[7px]"
            label={revealed ? `隐藏 ${apiKey.name} 的密钥` : `显示 ${apiKey.name} 的密钥`}
            onClick={() => setRevealed((value) => !value)}
          >
            {revealed ? <EyeOff size={13} /> : <Eye size={13} />}
          </IconButton>
          <IconButton
            size="sm"
            className="size-7 rounded-[7px]"
            label={`复制 ${apiKey.name} 的密钥`}
            onClick={() => void copyToken()}
          >
            {copied ? <Check size={13} /> : <Copy size={13} />}
          </IconButton>
        </div>
      </td>
      <td className="px-3 py-2.5 align-middle">
        <UsageStats name={apiKey.name} usage={apiKey.usage} />
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

function UsageStats({ name, usage }: { name: string; usage: GatewayApiKeyUsage }) {
  const rate = usage.totalRequests
    ? Math.round((usage.successfulRequests / usage.totalRequests) * 1_000) / 10
    : null;
  const outcomes = usage.recentOutcomes;
  const outcomeLabel = outcomes
    .map((outcome) => (isSuccess(outcome.statusCode) ? "成功" : `失败 ${outcome.statusCode}`))
    .join("、");

  return (
    <div className="min-w-[180px] space-y-1.5">
      <div className="flex flex-wrap items-center gap-1.5 text-[11px] tabular-nums">
        <span className="rounded-md bg-success/10 px-1.5 py-0.5 font-medium text-success">
          成功: {formatCount(usage.successfulRequests)}
        </span>
        <span className="rounded-md bg-danger/10 px-1.5 py-0.5 font-medium text-danger">
          失败: {formatCount(usage.failedRequests)}
        </span>
      </div>
      {rate === null ? (
        <p className="text-[11px] text-tertiary">暂无调用</p>
      ) : (
        <div className="flex items-center gap-2">
          <div
            className="flex min-w-0 flex-1 items-center gap-[3px]"
            role="img"
            aria-label={`${name} 最近 ${outcomes.length} 次调用：${outcomeLabel || "暂无结果"}`}
          >
            {outcomes.map((outcome, index) => (
              <span
                key={`${outcome.statusCode}-${index}`}
                className={`block size-[4px] shrink-0 rounded-[1px] ${outcomeTone(outcome.statusCode)}`}
                title={`HTTP ${outcome.statusCode}`}
              />
            ))}
          </div>
          <span className="shrink-0 rounded-md bg-danger/10 px-1.5 py-0.5 text-[11px] font-medium tabular-nums text-danger">
            {rate.toFixed(1)}%
          </span>
        </div>
      )}
    </div>
  );
}

function formatCount(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

function isSuccess(statusCode: number) {
  return statusCode >= 200 && statusCode < 300;
}

function outcomeTone(statusCode: number) {
  if (isSuccess(statusCode)) {
    return "bg-success";
  }
  if (statusCode >= 400 && statusCode < 500) {
    return "bg-warning";
  }
  return "bg-danger";
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

function formatTimestamp(value: string) {
  return value.replace(" ", " · ");
}
