import { Activity, Globe, LoaderCircle, Pencil, Trash2 } from "lucide-react";
import type { ReactNode } from "react";

import type { ProxyProfile, ProxyTestResult } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { formatProxyTestDiagnostic } from "./proxy-test-result";
import { cn } from "@/shared/lib/cn";
import { RowActionButton } from "@/shared/ui/RowActionButton";

export interface ProxyTableRowProps {
  proxy: ProxyProfile;
  isGlobal: boolean;
  pending: boolean;
  testing: boolean;
  testPending: boolean;
  testResult?: ProxyTestResult;
  testError: unknown;
  onTest: () => void;
  onEdit: (id: string) => void;
  onSetGlobal: (proxy: ProxyProfile) => void;
  onDelete: (proxy: ProxyProfile) => void;
}

const MOBILE_ICON_ACTION =
  "h-10 w-10 shrink-0 justify-center whitespace-nowrap px-0 sm:h-7 sm:w-auto sm:px-1.5";

export function ProxyTableRow({
  proxy,
  isGlobal,
  pending,
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
    <tr
      data-responsive-row="card"
      className="grid grid-cols-[minmax(0,1fr)_auto] overflow-hidden rounded-[8px] border border-subtle bg-surface sm:table-row sm:rounded-none sm:border-x-0 sm:border-t-0 sm:bg-transparent sm:last:border-b-0"
    >
      <td className="col-start-1 row-start-1 min-w-0 px-3 pb-2.5 pt-3 align-middle sm:table-cell sm:py-2.5 sm:pl-0 sm:pr-3">
        <div className="flex min-w-0 flex-wrap items-center gap-1.5">
          <p className="min-w-0 break-words font-medium text-primary [overflow-wrap:anywhere]">
            {proxy.name}
          </p>
          {isGlobal ? <GlobalRouteBadge /> : null}
        </div>
      </td>
      <td className="col-start-2 row-start-1 flex items-center justify-end px-3 pb-2.5 pt-3 align-middle sm:table-cell sm:px-3 sm:py-2.5">
        <Badge>{proxy.kind.toUpperCase()}</Badge>
      </td>
      <td className="col-span-2 row-start-2 grid min-w-0 grid-cols-1 gap-1 border-t border-subtle px-3 py-2.5 align-middle sm:table-cell sm:border-t-0 sm:px-3">
        <MobileFieldLabel>地址</MobileFieldLabel>
        <span className="min-w-0 break-words text-secondary [overflow-wrap:anywhere]">{endpoint}</span>
      </td>
      <td className="col-start-1 row-start-3 min-w-0 px-3 py-1.5 align-middle sm:table-cell sm:px-3 sm:py-2.5">
        <MobileFieldLabel>状态</MobileFieldLabel>
        <div className="mt-1 flex flex-wrap items-center gap-2 sm:mt-0">
          <ProxyStatus enabled={proxy.enabled} />
          {proxy.builtIn ? <span className="text-[11px] text-tertiary">内置</span> : null}
        </div>
      </td>
      <td className="col-start-2 row-start-3 min-w-0 px-3 py-1.5 align-middle sm:table-cell sm:px-3 sm:py-2.5 sm:text-secondary">
        <MobileFieldLabel>认证</MobileFieldLabel>
        <span className="mt-1 block break-words text-secondary [overflow-wrap:anywhere] sm:mt-0">
          {proxy.builtIn ? "—" : proxy.passwordConfigured ? proxy.username ?? "已配置" : "无"}
        </span>
      </td>
      <td className="col-span-2 row-start-4 grid min-w-0 grid-cols-[4.5rem_minmax(0,1fr)] items-center gap-3 px-3 pb-2.5 pt-1.5 align-middle sm:table-cell sm:w-[178px] sm:min-w-[178px] sm:max-w-[178px] sm:px-3 sm:py-2.5">
        <MobileFieldLabel>连通性</MobileFieldLabel>
        <div className="flex min-w-0 justify-end sm:block">
          <ProxyTestStatus testing={testing} result={testResult} error={testError} />
        </div>
      </td>
      <td className="col-span-2 row-start-5 border-t border-subtle px-2 py-1.5 align-middle sm:table-cell sm:border-t-0 sm:py-2.5 sm:pl-3 sm:pr-0">
        <div className="flex items-center justify-end gap-0.5">
          <RowActionButton
            label={`测试 ${proxy.name}`}
            title={`测试 ${proxy.name}`}
            className={MOBILE_ICON_ACTION}
            disabled={!proxy.enabled || testPending || pending}
            onClick={onTest}
          >
            {testing ? <LoaderCircle size={13} className="animate-spin" /> : <Activity size={13} />}
            <span className="sr-only sm:not-sr-only">测试</span>
          </RowActionButton>
          {!isGlobal && proxy.enabled ? (
            <RowActionButton
              label={`将 ${proxy.name} 设为全局出口`}
              title={`将 ${proxy.name} 设为全局出口`}
              className={MOBILE_ICON_ACTION}
              disabled={pending}
              onClick={() => onSetGlobal(proxy)}
            >
              <Globe size={13} />
              <span className="sr-only sm:not-sr-only">全局</span>
            </RowActionButton>
          ) : null}
          {!proxy.builtIn ? (
            <>
              <RowActionButton
                label={`编辑 ${proxy.name}`}
                title={`编辑 ${proxy.name}`}
                className={MOBILE_ICON_ACTION}
                disabled={pending}
                onClick={() => onEdit(proxy.id)}
              >
                <Pencil size={13} />
                <span className="sr-only sm:not-sr-only">编辑</span>
              </RowActionButton>
              <RowActionButton
                label={`删除 ${proxy.name}`}
                title={`删除 ${proxy.name}`}
                className={MOBILE_ICON_ACTION}
                disabled={pending || isGlobal}
                tone="danger"
                onClick={() => onDelete(proxy)}
              >
                <Trash2 size={13} />
                <span className="sr-only sm:not-sr-only">删除</span>
              </RowActionButton>
            </>
          ) : null}
        </div>
      </td>
    </tr>
  );
}

function MobileFieldLabel({ children }: { children: ReactNode }) {
  return (
    <span className="text-[11px] font-medium text-tertiary sm:hidden">
      {children}
    </span>
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
  let status = "未测试";
  let latency = "—";
  let tone: ProxyTestTone = "neutral";
  let diagnostic = "尚未测试公网连通性";

  if (testing) {
    status = "测试中";
    tone = "progress";
    diagnostic = "正在测试公网连通性";
  } else if (error) {
    status = "失败";
    tone = "danger";
    diagnostic = getProxyErrorMessage(error);
  } else if (result) {
    status = result.reachable ? "成功" : "失败";
    latency = `${result.latencyMs} ms`;
    tone = result.reachable ? "success" : "danger";
    diagnostic = formatProxyTestDiagnostic(result);
  }

  return (
    <div
      className="grid h-6 w-[154px] shrink-0 grid-cols-[64px_84px] items-center gap-1.5 text-[11px] font-medium"
      role={error ? "alert" : "status"}
      aria-label={diagnostic}
      title={diagnostic}
      data-testid="proxy-test-status"
    >
      <span className={cn("text-center", proxyTestToneClass(tone))}>{status}</span>
      <span className="text-center text-tertiary tabular-nums">{latency}</span>
    </div>
  );
}

type ProxyTestTone = "neutral" | "progress" | "success" | "danger";

function proxyTestToneClass(tone: ProxyTestTone) {
  switch (tone) {
    case "progress":
      return "text-accent-copy";
    case "success":
      return "text-success";
    case "danger":
      return "text-danger";
    case "neutral":
      return "text-secondary";
  }
}

/** Active global route marker — visible next to the name for quick scan. */
function GlobalRouteBadge() {
  return (
    <span
      className="inline-flex shrink-0 items-center gap-1 text-[11px] font-medium text-accent-copy"
      title="当前全局出口：Credential 绑定 DIRECT 时继承此出口"
    >
      <Globe size={11} strokeWidth={2.25} aria-hidden="true" />
      全局路由
    </span>
  );
}

function ProxyStatus({ enabled }: { enabled: boolean }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 text-[11px]",
        enabled ? "text-success" : "text-tertiary",
      )}
    >
      <span
        className={cn("size-1.5 rounded-full", enabled ? "bg-success" : "bg-tertiary")}
      />
      {enabled ? "已启用" : "已停用"}
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
