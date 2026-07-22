import { Pencil, Trash2, X } from "lucide-react";
import { useState } from "react";

import type { ProxyProfile } from "../api/proxy-contracts";
import { cn } from "@/shared/lib/cn";

export interface ProxyTableRowProps {
  proxy: ProxyProfile;
  isGlobal: boolean;
  pending: boolean;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
}

export function ProxyTableRow({
  proxy,
  isGlobal,
  pending,
  onEdit,
  onDelete,
}: ProxyTableRowProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const endpoint = proxy.host && proxy.port ? `${proxy.host}:${proxy.port}` : "本机网络";

  return (
    <tr className="border-b border-subtle last:border-b-0">
      <td className="py-2.5 pr-3 align-middle">
        <p className="break-words font-medium text-primary [overflow-wrap:anywhere]">{proxy.name}</p>
      </td>
      <td className="px-3 py-2.5 align-middle">
        <Badge>{proxy.kind.toUpperCase()}</Badge>
      </td>
      <td className="px-3 py-2.5 align-middle">
        <span className="break-all text-secondary">{endpoint}</span>
      </td>
      <td className="px-3 py-2.5 align-middle">
        <div className="flex flex-wrap gap-1.5">
          {proxy.enabled ? <Badge tone="success">已启用</Badge> : <Badge>已停用</Badge>}
          {isGlobal ? <Badge tone="accent">全局</Badge> : null}
          {proxy.builtIn ? <Badge>内置</Badge> : null}
        </div>
      </td>
      <td className="px-3 py-2.5 align-middle text-secondary">
        {proxy.builtIn ? "—" : proxy.passwordConfigured ? proxy.username ?? "已配置" : "无"}
      </td>
      <td className="py-2.5 pl-3 align-middle">
        <div className="flex flex-wrap items-center justify-end gap-0.5">
          {!proxy.builtIn ? (
            <>
              <RowAction label={`编辑 ${proxy.name}`} disabled={pending} onClick={() => onEdit(proxy.id)}>
                <Pencil size={13} />
                编辑
              </RowAction>
              {confirmDelete ? (
                <>
                  <RowAction
                    label={`确认删除 ${proxy.name}`}
                    disabled={pending || isGlobal}
                    tone="danger"
                    onClick={() => onDelete(proxy.id)}
                  >
                    <Trash2 size={13} />
                    确认删除
                  </RowAction>
                  <RowAction
                    label={`取消删除 ${proxy.name}`}
                    disabled={pending}
                    onClick={() => setConfirmDelete(false)}
                  >
                    <X size={13} />
                    取消
                  </RowAction>
                </>
              ) : (
                <RowAction
                  label={`删除 ${proxy.name}`}
                  disabled={pending || isGlobal}
                  tone="danger"
                  onClick={() => setConfirmDelete(true)}
                >
                  <Trash2 size={13} />
                  删除
                </RowAction>
              )}
            </>
          ) : null}
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
  tone?: "neutral" | "success" | "accent";
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] font-medium",
        tone === "success" && "bg-success/10 text-success",
        tone === "accent" && "bg-surface-muted text-primary",
        tone === "neutral" && "bg-surface-muted text-secondary",
      )}
    >
      {children}
    </span>
  );
}
