import { KeyRound, Pencil, Trash2 } from "lucide-react";
import { Link } from "react-router-dom";

import type { ProviderEndpoint } from "../api/provider-contracts";
import { cn } from "@/shared/lib/cn";

export interface ProviderEndpointTableRowProps {
  endpoint: ProviderEndpoint;
  pending: boolean;
  onEdit: (id: string) => void;
  onDelete: (endpoint: ProviderEndpoint) => void;
}

export function ProviderEndpointTableRow({
  endpoint,
  pending,
  onEdit,
  onDelete,
}: ProviderEndpointTableRowProps) {
  const dialect =
    endpoint.protocolDialect === "openai_responses" ? "Responses" : "Messages";
  const hasRisk = endpoint.allowInsecureHttp || endpoint.allowPrivateNetwork;

  return (
    <tr className="border-b border-subtle last:border-b-0">
      <td className="py-2.5 pr-3 align-middle">
        <p className="break-words font-medium text-primary [overflow-wrap:anywhere]">{endpoint.name}</p>
      </td>
      <td className="px-3 py-2.5 align-middle">
        <div className="flex flex-wrap gap-1.5">
          <Badge>{endpoint.providerKind.toUpperCase()}</Badge>
          <Badge>{dialect}</Badge>
        </div>
      </td>
      <td className="px-3 py-2.5 align-middle">
        <span className="break-all text-secondary">{endpoint.baseUrl}</span>
      </td>
      <td className="px-3 py-2.5 align-middle">
        <div className="flex flex-wrap gap-1.5">
          {endpoint.enabled ? <Badge tone="success">已启用</Badge> : <Badge>已停用</Badge>}
          {hasRisk ? <Badge tone="warning">显式授权</Badge> : null}
        </div>
      </td>
      <td className="py-2.5 pl-3 align-middle">
        <div className="flex flex-wrap items-center justify-end gap-0.5">
          <Link
            className={rowLinkClass}
            aria-label={`管理 ${endpoint.name} 的 API Key`}
            to={`/providers/${encodeURIComponent(endpoint.id)}`}
          >
            <KeyRound size={13} />
            API Key
          </Link>
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
      </td>
    </tr>
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

function Badge({
  children,
  tone = "neutral",
}: {
  children: React.ReactNode;
  tone?: "neutral" | "success" | "warning";
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] font-medium",
        tone === "success" && "bg-success/10 text-success",
        tone === "warning" && "bg-warning/12 text-warning",
        tone === "neutral" && "bg-surface-muted text-secondary",
      )}
    >
      {children}
    </span>
  );
}

const rowLinkClass =
  "focus-ring inline-flex h-7 items-center gap-1 rounded-[7px] px-2 text-[12px] font-medium text-secondary transition-colors hover:bg-surface-muted hover:text-primary";
