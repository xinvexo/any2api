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
  "h-10 w-10 shrink-0 justify-center whitespace-nowrap px-0 sm:h-7 sm:w-auto sm:px-1.5";

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
      className="grid grid-cols-[minmax(0,1fr)_auto] overflow-hidden rounded-[8px] border border-subtle bg-surface sm:table-row sm:rounded-none sm:border-x-0 sm:border-t-0 sm:bg-transparent sm:last:border-b-0"
    >
      <td className="col-start-1 row-start-1 min-w-0 px-3 pb-2.5 pt-3 align-middle sm:table-cell sm:py-2.5 sm:pl-0 sm:pr-3">
        <p className="break-words font-medium text-primary [overflow-wrap:anywhere]">{apiKey.name}</p>
      </td>
      <td className="col-span-2 row-start-2 min-w-0 border-t border-subtle px-3 py-2.5 align-middle sm:table-cell sm:border-t-0">
        <MobileFieldLabel>调用统计</MobileFieldLabel>
        <RequestUsageStats
          className="mt-2 flex-wrap items-center gap-x-1.5 gap-y-2 sm:mt-0 sm:flex-nowrap sm:gap-2.5"
          label={apiKey.name}
          usage={apiKey.usage}
        />
      </td>
      <td className="col-start-2 row-start-1 flex items-center justify-end px-3 pb-2.5 pt-3 align-middle sm:table-cell sm:px-3 sm:py-2.5">
        <span title={apiKey.enabled ? "已启用" : "已停用"}>
          <Switch
            checked={apiKey.enabled}
            disabled={pending}
            aria-label={apiKey.enabled ? `禁用 ${apiKey.name}` : `启用 ${apiKey.name}`}
            onCheckedChange={() => onToggleEnabled(apiKey)}
          />
        </span>
      </td>
      <td className="col-span-2 row-start-3 grid min-w-0 grid-cols-[5rem_minmax(0,1fr)] items-baseline gap-3 px-3 py-1.5 align-middle sm:table-cell sm:px-3 sm:py-2.5 sm:text-secondary sm:tabular-nums">
        <MobileFieldLabel>最后使用</MobileFieldLabel>
        <span className="min-w-0 text-right text-secondary tabular-nums sm:text-left">
          {apiKey.lastUsedAt ? formatTimestamp(apiKey.lastUsedAt) : "—"}
        </span>
      </td>
      <td className="col-span-2 row-start-4 grid min-w-0 grid-cols-[5rem_minmax(0,1fr)] items-baseline gap-3 px-3 pb-2.5 pt-1.5 align-middle sm:table-cell sm:px-3 sm:py-2.5 sm:text-secondary sm:tabular-nums">
        <MobileFieldLabel>创建时间</MobileFieldLabel>
        <span className="min-w-0 text-right text-secondary tabular-nums sm:text-left">
          {formatTimestamp(apiKey.createdAt)}
        </span>
      </td>
      <td className="col-span-2 row-start-5 border-t border-subtle px-2 py-1.5 align-middle sm:table-cell sm:border-t-0 sm:py-2.5 sm:pl-3 sm:pr-0">
        <div className="flex items-center justify-end gap-0.5">
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
