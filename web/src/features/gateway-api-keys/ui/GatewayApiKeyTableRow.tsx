import { Copy, Pencil, RefreshCw, Trash2 } from "lucide-react";

import type { GatewayApiKey } from "../api/gateway-api-key-contracts";
import { notify } from "@/shared/notifications";
import { RequestUsageStats } from "@/shared/ui/RequestUsageStats";
import { RowActionButton } from "@/shared/ui/RowActionButton";
import { Switch } from "@/shared/ui/Switch";

export interface GatewayApiKeyTableRowProps {
  apiKey: GatewayApiKey;
  pending: boolean;
  onEdit: (id: string) => void;
  onToggleEnabled: (key: GatewayApiKey) => void;
  onRotate: (key: GatewayApiKey) => void;
  onDelete: (key: GatewayApiKey) => void;
}

const MOBILE_ICON_ACTION =
  "h-9 w-9 shrink-0 justify-center whitespace-nowrap rounded-full px-0 sm:h-7 sm:w-auto sm:rounded-[7px] sm:px-1.5";

export function GatewayApiKeyTableRow({
  apiKey,
  pending,
  onEdit,
  onToggleEnabled,
  onRotate,
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
    <tr
      data-floating-bounds
      data-responsive-row="card"
      className="desktop-table-card-row grid grid-cols-1 rounded-[14px] bg-surface-muted/55 p-3 sm:table-row sm:rounded-none sm:bg-transparent sm:p-0"
    >
      <td className="min-w-0 pr-14 align-middle sm:table-cell sm:px-3 sm:py-2.5">
        <p className="break-words text-[13px] font-semibold tracking-tight text-primary [overflow-wrap:anywhere] sm:text-[12px] sm:font-medium sm:tracking-normal">
          {apiKey.name}
        </p>
      </td>
      <td className="min-w-0 pt-2.5 align-middle sm:table-cell sm:px-3 sm:py-2.5">
        <MobileFieldLabel>调用统计</MobileFieldLabel>
        <RequestUsageStats
          className="mt-1.5 flex-wrap items-center gap-x-1.5 gap-y-2 sm:mt-0 sm:flex-nowrap sm:gap-2.5"
          label={apiKey.name}
          unitLabel="客户端请求"
          usage={apiKey.usage}
        />
      </td>
      <td className="grid min-w-0 grid-cols-[5rem_minmax(0,1fr)] items-baseline gap-3 pt-2.5 align-middle sm:table-cell sm:px-3 sm:py-2.5 sm:text-secondary sm:tabular-nums">
        <MobileFieldLabel>最后使用</MobileFieldLabel>
        <span className="min-w-0 text-right text-secondary tabular-nums sm:text-left">
          {apiKey.lastUsedAt ? formatTimestamp(apiKey.lastUsedAt) : "—"}
        </span>
      </td>
      <td className="grid min-w-0 grid-cols-[5rem_minmax(0,1fr)] items-baseline gap-3 pt-1.5 align-middle sm:table-cell sm:px-3 sm:py-2.5 sm:text-secondary sm:tabular-nums">
        <MobileFieldLabel>创建时间</MobileFieldLabel>
        <span className="min-w-0 text-right text-secondary tabular-nums sm:text-left">
          {formatTimestamp(apiKey.createdAt)}
        </span>
      </td>
      <td className="pt-1.5 align-middle sm:table-cell sm:w-80 sm:min-w-80 sm:px-3 sm:py-2.5">
        <div className="flex items-center justify-end gap-0.5 whitespace-nowrap sm:w-full sm:gap-1.5">
          <span
            className="absolute right-3 top-3 inline-flex h-7 items-center sm:static sm:h-7"
            title={apiKey.enabled ? "已启用" : "已停用"}
          >
            <Switch
              checked={apiKey.enabled}
              disabled={pending}
              aria-label={apiKey.enabled ? `禁用 ${apiKey.name}` : `启用 ${apiKey.name}`}
              onCheckedChange={() => onToggleEnabled(apiKey)}
            />
          </span>
          <RowActionButton
            label={`复制 ${apiKey.name} 的密钥`}
            title={`复制 ${apiKey.name} 的密钥`}
            className={MOBILE_ICON_ACTION}
            disabled={pending}
            onClick={() => void copyToken()}
          >
            <Copy size={13} />
            <span className="sr-only sm:not-sr-only">复制</span>
          </RowActionButton>
          <RowActionButton
            label={`编辑 ${apiKey.name}`}
            title={`编辑 ${apiKey.name}`}
            className={MOBILE_ICON_ACTION}
            disabled={pending}
            onClick={() => onEdit(apiKey.id)}
          >
            <Pencil size={13} />
            <span className="sr-only sm:not-sr-only">编辑</span>
          </RowActionButton>
          <RowActionButton
            label={`轮换 ${apiKey.name} 的密钥`}
            title={`轮换 ${apiKey.name} 的密钥`}
            className={MOBILE_ICON_ACTION}
            disabled={pending}
            onClick={() => onRotate(apiKey)}
          >
            <RefreshCw size={13} />
            <span className="sr-only sm:not-sr-only">轮换</span>
          </RowActionButton>
          <RowActionButton
            label={`删除 ${apiKey.name}`}
            title={`删除 ${apiKey.name}`}
            className={MOBILE_ICON_ACTION}
            disabled={pending}
            tone="danger"
            onClick={() => onDelete(apiKey)}
          >
            <Trash2 size={13} />
            <span className="sr-only sm:not-sr-only">删除</span>
          </RowActionButton>
        </div>
      </td>
    </tr>
  );
}

function MobileFieldLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-[11px] font-medium text-tertiary sm:hidden">
      {children}
    </span>
  );
}

function formatTimestamp(value: string) {
  return value.replace(" ", " · ");
}
