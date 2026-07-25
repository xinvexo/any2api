import { Ban, Copy, Pencil, Power, Trash2 } from "lucide-react";

import type { GatewayApiKey, GatewayApiKeyUsage } from "../api/gateway-api-key-contracts";
import { notify } from "@/shared/notifications";
import { cn } from "@/shared/lib/cn";
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

/** Matches storage `GATEWAY_API_KEY_RECENT_OUTCOME_LIMIT` — fixed width, no row jitter. */
const RECENT_OUTCOME_SLOTS = 24;

function UsageStats({ name, usage }: { name: string; usage: GatewayApiKeyUsage }) {
  const outcomes = usage.recentOutcomes;
  // Left-pad empty slots so the bar is always RECENT_OUTCOME_SLOTS wide (newest on the right).
  const slots: Array<number | null> = [
    ...Array.from<number | null>({
      length: Math.max(0, RECENT_OUTCOME_SLOTS - outcomes.length),
    }).fill(null),
    ...outcomes.map((outcome) => outcome.statusCode),
  ].slice(-RECENT_OUTCOME_SLOTS);
  const outcomeLabel = outcomes
    .map((outcome) => (isSuccess(outcome.statusCode) ? "成功" : `失败 ${outcome.statusCode}`))
    .join("、");

  return (
    <div className="flex min-w-0 max-w-full items-center gap-2">
      <div className="flex shrink-0 items-center gap-x-2 text-[11px] tabular-nums">
        <span className="font-medium text-success">
          成功 {formatCount(usage.successfulRequests)}
        </span>
        <span className="font-medium text-danger">
          失败 {formatCount(usage.failedRequests)}
        </span>
      </div>
      <div
        className="flex h-4 w-full min-w-[9rem] max-w-[16rem] flex-1 items-stretch gap-px"
        role="img"
        aria-label={`${name} 最近 ${outcomes.length} 次调用：${outcomeLabel || "暂无调用"}`}
      >
        {slots.map((statusCode, index) => (
          <span
            key={`slot-${index}`}
            className={cn("min-w-[2px] flex-1 rounded-[2px]", outcomeSlotTone(statusCode))}
            title={statusCode === null ? "无记录" : `HTTP ${statusCode}`}
          />
        ))}
      </div>
    </div>
  );
}

function formatCount(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

function isSuccess(statusCode: number) {
  return statusCode >= 200 && statusCode < 300;
}

function outcomeSlotTone(statusCode: number | null) {
  if (statusCode === null) {
    return "bg-black/[0.08] dark:bg-white/[0.12]";
  }
  if (isSuccess(statusCode)) {
    return "bg-success/85";
  }
  if (statusCode >= 400 && statusCode < 500) {
    return "bg-warning/85";
  }
  return "bg-danger/85";
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
