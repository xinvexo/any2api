import { Activity, Globe, LoaderCircle, Pencil, Trash2 } from "lucide-react";
import type { ReactNode } from "react";

import type { ProxyProfile, ProxyTestResult } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { formatProxyTestResult } from "./proxy-test-result";
import { cn } from "@/shared/lib/cn";
import { RowActionButton } from "@/shared/ui/RowActionButton";

export interface ProxyTableRowProps {
  proxy: ProxyProfile;
  isGlobal: boolean;
  pending: boolean;
  canTest: boolean;
  testing: boolean;
  testPending: boolean;
  testResult?: ProxyTestResult;
  testError: unknown;
  onTest: () => void;
  onEdit: (id: string) => void;
  onSetGlobal: (proxy: ProxyProfile) => void;
  onDelete: (proxy: ProxyProfile) => void;
}

export function ProxyTableRow({
  proxy,
  isGlobal,
  pending,
  canTest,
  testing,
  testPending,
  testResult,
  testError,
  onTest,
  onEdit,
  onSetGlobal,
  onDelete,
}: ProxyTableRowProps) {
  const endpoint = proxy.host && proxy.port ? `${proxy.host}:${proxy.port}` : "本机网络";

  return (
    <tr className="border-b border-subtle last:border-b-0">
      <td className="py-2.5 pr-3 align-middle">
        <div className="flex min-w-0 flex-wrap items-center gap-1.5">
          <p className="min-w-0 break-words font-medium text-primary [overflow-wrap:anywhere]">
            {proxy.name}
          </p>
          {isGlobal ? <GlobalRouteBadge /> : null}
        </div>
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
          {proxy.builtIn ? <Badge>内置</Badge> : null}
        </div>
      </td>
      <td className="px-3 py-2.5 align-middle text-secondary">
        {proxy.builtIn ? "—" : proxy.passwordConfigured ? proxy.username ?? "已配置" : "无"}
      </td>
      <td className="px-3 py-2.5 align-middle">
        <ProxyTestStatus testing={testing} result={testResult} error={testError} />
      </td>
      <td className="py-2.5 pl-3 align-middle">
        <div className="flex flex-wrap items-center justify-end gap-0.5">
          <RowActionButton
            label={`测试 ${proxy.name}`}
            disabled={!canTest || !proxy.enabled || testPending || pending}
            onClick={onTest}
          >
            {testing ? <LoaderCircle size={13} className="animate-spin" /> : <Activity size={13} />}
            {testing ? "测试中" : "测试"}
          </RowActionButton>
          {!isGlobal && proxy.enabled ? (
            <RowActionButton
              label={`将 ${proxy.name} 设为全局出口`}
              disabled={pending}
              onClick={() => onSetGlobal(proxy)}
            >
              <Globe size={13} />
              全局
            </RowActionButton>
          ) : null}
          {!proxy.builtIn ? (
            <>
              <RowActionButton
                label={`编辑 ${proxy.name}`}
                disabled={pending}
                onClick={() => onEdit(proxy.id)}
              >
                <Pencil size={13} />
                编辑
              </RowActionButton>
              <RowActionButton
                label={`删除 ${proxy.name}`}
                disabled={pending || isGlobal}
                tone="danger"
                onClick={() => onDelete(proxy)}
              >
                <Trash2 size={13} />
                删除
              </RowActionButton>
            </>
          ) : null}
        </div>
      </td>
    </tr>
  );
}

function ProxyTestStatus({
  testing,
  result,
  error,
}: {
  testing: boolean;
  result?: ProxyTestResult;
  error: unknown;
}) {
  if (testing) {
    return <span className="text-secondary">正在测试</span>;
  }
  if (error) {
    return (
      <span className="block max-w-56 break-words text-danger" role="alert">
        {getProxyErrorMessage(error)}
      </span>
    );
  }
  if (!result) {
    return <span className="text-tertiary">—</span>;
  }
  return (
    <span className={result.reachable ? "text-success" : "text-danger"}>
      {formatProxyTestResult(result)}
    </span>
  );
}

/** Active global route marker — visible next to the name for quick scan. */
function GlobalRouteBadge() {
  return (
    <span
      className="inline-flex shrink-0 items-center gap-1 rounded-md bg-accent/10 px-1.5 py-0.5 text-[11px] font-medium text-accent-copy"
      title="当前全局出口：Credential 绑定 DIRECT 时继承此出口"
    >
      <Globe size={11} strokeWidth={2.25} aria-hidden="true" />
      全局路由
    </span>
  );
}

function Badge({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: "neutral" | "success";
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] font-medium",
        tone === "success" && "bg-success/10 text-success",
        tone === "neutral" && "bg-surface-muted text-secondary",
      )}
    >
      {children}
    </span>
  );
}
