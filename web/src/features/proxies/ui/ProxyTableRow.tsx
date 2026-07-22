import { Activity, Pencil, Trash2, X } from "lucide-react";
import { useState } from "react";

import type { ProxyProfile, ProxyTestResult } from "../api/proxy-contracts";
import { cn } from "@/shared/lib/cn";

export interface ProxyTableRowProps {
  proxy: ProxyProfile;
  isGlobal: boolean;
  pending: boolean;
  onEdit: (id: string) => void;
  onSetGlobal: (id: string) => void;
  onDelete: (id: string) => void;
  canTest: boolean;
  testing: boolean;
  testPending: boolean;
  testResult?: ProxyTestResult;
  onTest: () => void;
}

export function ProxyTableRow({
  proxy,
  isGlobal,
  pending,
  onEdit,
  onSetGlobal,
  onDelete,
  canTest,
  testing,
  testPending,
  testResult,
  onTest,
}: ProxyTableRowProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const endpoint = proxy.host && proxy.port ? `${proxy.host}:${proxy.port}` : "本机网络";

  return (
    <tr className="border-b border-subtle last:border-b-0">
      <td className="py-3.5 pr-3 align-middle">
        <div className="min-w-0">
          <p className="break-words font-medium text-primary [overflow-wrap:anywhere]">{proxy.name}</p>
          {testResult ? <TestResult result={testResult} /> : null}
        </div>
      </td>
      <td className="px-3 py-3.5 align-middle">
        <Badge>{proxy.kind.toUpperCase()}</Badge>
      </td>
      <td className="px-3 py-3.5 align-middle">
        <span className="break-all text-secondary">{endpoint}</span>
      </td>
      <td className="px-3 py-3.5 align-middle">
        <div className="flex flex-wrap gap-1.5">
          {proxy.enabled ? <Badge tone="success">已启用</Badge> : <Badge>已停用</Badge>}
          {isGlobal ? <Badge tone="accent">全局</Badge> : null}
          {proxy.builtIn ? <Badge>内置</Badge> : null}
        </div>
      </td>
      <td className="px-3 py-3.5 align-middle text-secondary">
        {proxy.builtIn ? "—" : proxy.passwordConfigured ? proxy.username ?? "已配置" : "无"}
      </td>
      <td className="py-3.5 pl-3 align-middle">
        <div className="flex flex-wrap items-center justify-end gap-1">
          <RowAction
            label={`测试 ${proxy.name}`}
            disabled={!canTest || !proxy.enabled || testPending || pending}
            onClick={onTest}
          >
            <Activity size={14} className={testing ? "animate-pulse" : undefined} />
            {testing ? "测试中" : "测试"}
          </RowAction>
          {!proxy.builtIn ? (
            <>
              {!isGlobal && proxy.enabled ? (
                <RowAction
                  label={`设为全局 ${proxy.name}`}
                  disabled={pending}
                  onClick={() => onSetGlobal(proxy.id)}
                >
                  设为全局
                </RowAction>
              ) : null}
              <RowAction label={`编辑 ${proxy.name}`} disabled={pending} onClick={() => onEdit(proxy.id)}>
                <Pencil size={14} />
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
                    <Trash2 size={14} />
                    确认删除
                  </RowAction>
                  <RowAction
                    label={`取消删除 ${proxy.name}`}
                    disabled={pending}
                    onClick={() => setConfirmDelete(false)}
                  >
                    <X size={14} />
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
                  <Trash2 size={14} />
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
        "focus-ring inline-flex h-8 items-center gap-1 rounded-[8px] px-2 text-[13px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        tone === "danger"
          ? "text-danger hover:bg-danger/8"
          : "text-secondary hover:bg-surface-muted hover:text-primary",
      )}
    >
      {children}
    </button>
  );
}

function TestResult({ result }: { result: ProxyTestResult }) {
  return result.reachable ? (
    <p className="mt-1 text-[11px] text-success">
      可达 · HTTP {result.statusCode} · {result.latencyMs} ms
    </p>
  ) : (
    <p className="mt-1 text-[11px] text-danger">
      失败 · {result.errorStage ?? "unknown"} · {result.failureScope ?? "unknown"}
    </p>
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
