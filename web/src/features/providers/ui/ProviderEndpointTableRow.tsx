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
    <div className="flex flex-col gap-1 sm:flex-row sm:items-center">
      <button
        type="button"
        className={cn(
          ENDPOINT_CONTENT_GRID_CLASS,
          "focus-ring min-w-0 flex-1 items-center rounded-[8px] py-1 text-left hover:bg-surface-muted/80",
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
            size={15}
            className={cn(
              "transition-transform duration-150",
              expanded && "rotate-90",
            )}
          />
        </span>
        <span className="min-w-0" aria-hidden="true">
          <span className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-0.5">
            <span className="truncate text-[13px] font-semibold tracking-tight text-primary">
              {endpoint.name}
            </span>
            <span className="text-[11px] text-tertiary">
              {dialect}
              {endpoint.enabled ? "" : " · 已停用"}
            </span>
          </span>
          <span className="mt-0.5 block truncate font-mono text-[11px] text-secondary">
            {endpoint.baseUrl}
          </span>
        </span>
      </button>
      <div className="flex w-full shrink-0 items-center justify-end gap-0.5 sm:w-auto">
        <RowActionButton
          label={`新增 ${endpoint.name} 的 API Key`}
          disabled={pending}
          onClick={() => onCreateCredential(endpoint.id)}
        >
          <Plus size={13} />
          新增
        </RowActionButton>
        <RowActionButton label={`编辑 ${endpoint.name}`} disabled={pending} onClick={() => onEdit(endpoint.id)}>
          <Pencil size={13} />
          编辑
        </RowActionButton>
        <RowActionButton
          label={`删除 ${endpoint.name}`}
          disabled={pending}
          tone="danger"
          onClick={() => onDelete(endpoint)}
        >
          <Trash2 size={13} />
          删除
        </RowActionButton>
      </div>
    </div>
  );
}
