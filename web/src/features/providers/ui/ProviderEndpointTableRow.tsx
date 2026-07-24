import { ChevronRight, Pencil, Plus, Trash2 } from "lucide-react";

import type { ProviderEndpoint } from "../api/provider-contracts";
import { protocolLabel } from "../model/protocol-catalog";
import { cn } from "@/shared/lib/cn";
import { RowActionButton } from "@/shared/ui/RowActionButton";

/**
 * Shared by endpoint header and nested key list:
 * col1 = chevron gutter (1rem), col2 = title / URL / key table.
 */
export const ENDPOINT_CONTENT_GRID_CLASS =
  "grid grid-cols-[1rem_minmax(0,1fr)] gap-x-2";

export interface ProviderEndpointTableRowProps {
  endpoint: ProviderEndpoint;
  pending: boolean;
  expanded: boolean;
  onToggle: () => void;
  onEdit: (id: string) => void;
  onCreateCredential: (endpointId: string) => void;
  onDelete: (endpoint: ProviderEndpoint) => void;
}

export function ProviderEndpointTableRow({
  endpoint,
  pending,
  expanded,
  onToggle,
  onEdit,
  onCreateCredential,
  onDelete,
}: ProviderEndpointTableRowProps) {
  const accepted = protocolLabel(endpoint.protocolDialect);
  const dialect = endpoint.upstreamProtocolDialect
    ? `${accepted} → ${protocolLabel(endpoint.upstreamProtocolDialect)}`
    : accepted;
  const panelId = `endpoint-keys-${endpoint.id}`;

  return (
    <div className="flex min-w-0 flex-col gap-1.5 overflow-hidden sm:flex-row sm:items-start sm:gap-2">
      <button
        type="button"
        className={cn(
          ENDPOINT_CONTENT_GRID_CLASS,
          "focus-ring min-w-0 flex-1 items-center rounded-[8px] py-0.5 text-left",
          "hover:bg-surface-muted/50 active:bg-surface-muted/70",
        )}
        aria-expanded={expanded}
        aria-controls={panelId}
        aria-label={`${expanded ? "收起" : "展开"} ${endpoint.name} 的 API Key`}
        onClick={onToggle}
      >
        <span
          className="inline-flex size-4 shrink-0 items-center justify-center text-tertiary"
          aria-hidden="true"
        >
          <ChevronRight
            size={14}
            className={cn(
              "transition-transform duration-150",
              expanded && "rotate-90",
            )}
          />
        </span>
        <span className="min-w-0" aria-hidden="true">
          <span className="flex min-w-0 flex-wrap items-center gap-x-1.5 gap-y-0.5">
            <span className="truncate text-[13px] font-semibold tracking-tight text-primary">
              {endpoint.name}
            </span>
            <span className="shrink-0 rounded-full bg-surface-muted px-1.5 py-px text-[10px] font-medium leading-4 text-secondary">
              {dialect}
            </span>
            {!endpoint.enabled ? (
              <span className="shrink-0 rounded-full bg-warning/12 px-1.5 py-px text-[10px] font-medium leading-4 text-warning">
                已停用
              </span>
            ) : null}
          </span>
          <span className="mt-0.5 block truncate font-mono text-[11px] leading-4 text-tertiary">
            {endpoint.baseUrl}
          </span>
        </span>
      </button>
      <div className="flex min-w-0 flex-wrap items-center justify-end gap-0.5 sm:max-w-[40%] sm:shrink-0 sm:pl-0">
        <RowActionButton
          quiet
          label={`新增 ${endpoint.name} 的 API Key`}
          disabled={pending}
          onClick={() => onCreateCredential(endpoint.id)}
        >
          <Plus size={12} />
          新增
        </RowActionButton>
        <RowActionButton
          quiet
          label={`编辑 ${endpoint.name}`}
          disabled={pending}
          onClick={() => onEdit(endpoint.id)}
        >
          <Pencil size={12} />
          编辑
        </RowActionButton>
        <RowActionButton
          quiet
          label={`删除 ${endpoint.name}`}
          disabled={pending}
          tone="danger"
          onClick={() => onDelete(endpoint)}
        >
          <Trash2 size={12} />
        </RowActionButton>
      </div>
    </div>
  );
}
