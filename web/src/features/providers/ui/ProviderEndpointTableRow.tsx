import { ChevronRight, Pencil, Plus, Trash2 } from "lucide-react";

import type { ProviderEndpoint } from "../api/provider-contracts";
import { cn } from "@/shared/lib/cn";

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
  const dialect =
    endpoint.protocolDialect === "openai_responses" ? "Responses" : "Messages";
  const panelId = `endpoint-keys-${endpoint.id}`;

  return (
    <div className="flex flex-col gap-1 sm:flex-row sm:items-center">
      <button
        type="button"
        className="focus-ring flex w-full min-w-0 flex-1 items-center gap-2 rounded-[8px] px-1 py-1 text-left hover:bg-surface-muted/80"
        aria-expanded={expanded}
        aria-controls={panelId}
        aria-label={`${expanded ? "收起" : "展开"} ${endpoint.name} 的 API Key`}
        onClick={onToggle}
      >
        <ChevronRight
          size={15}
          className={cn(
            "shrink-0 text-tertiary transition-transform duration-150",
            expanded && "rotate-90",
          )}
          aria-hidden="true"
        />
        <span className="min-w-0 flex-1" aria-hidden="true">
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
        <RowAction
          label={`新增 ${endpoint.name} 的 API Key`}
          disabled={pending}
          onClick={() => onCreateCredential(endpoint.id)}
        >
          <Plus size={13} />
          新增
        </RowAction>
        <RowAction label={`编辑 ${endpoint.name}`} disabled={pending} onClick={() => onEdit(endpoint.id)}>
          <Pencil size={13} />
          编辑
        </RowAction>
        <RowAction
          label={`删除 ${endpoint.name}`}
          disabled={pending}
          tone="danger"
          onClick={() => onDelete(endpoint)}
        >
          <Trash2 size={13} />
          删除
        </RowAction>
      </div>
    </div>
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
