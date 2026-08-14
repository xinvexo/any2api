import {
  Activity,
  Globe,
  LoaderCircle,
  LockKeyhole,
  Network,
  Pencil,
  Trash2,
} from "lucide-react";
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
  "h-9 w-9 shrink-0 justify-center whitespace-nowrap rounded-full px-0 sm:h-7 sm:w-auto sm:rounded-[7px] sm:px-1.5";

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
  const authentication = proxy.builtIn
    ? "无需认证"
    : proxy.passwordConfigured
      ? proxy.username ?? "已配置认证"
      : "无认证";

  return (
    <tr
      data-responsive-row="card"
      className="grid grid-cols-[minmax(0,1fr)_auto] rounded-[14px] bg-surface-muted/55 p-3 transition-colors sm:table-row sm:rounded-none sm:border-b sm:border-subtle/50 sm:bg-transparent sm:p-0 sm:last:border-b-0 sm:hover:bg-surface-muted/20"
    >
      <td className="col-start-1 row-start-1 min-w-0 align-middle sm:table-cell sm:py-2.5 sm:pl-0 sm:pr-3">
        <div className="flex min-w-0 flex-wrap items-center gap-1.5">
          <p className="min-w-0 break-words text-[13px] font-semibold tracking-tight text-primary [overflow-wrap:anywhere] sm:text-[12px] sm:font-medium sm:tracking-normal">
            {proxy.name}
          </p>
          {isGlobal ? <GlobalRouteBadge /> : null}
        </div>
      </td>
      <td className="col-start-2 row-start-1 flex items-center justify-end align-middle sm:table-cell sm:px-3 sm:py-2.5">
        <Badge>{proxy.kind.toUpperCase()}</Badge>
      </td>
      <td className="col-span-2 row-start-2 min-w-0 pt-1.5 align-middle sm:table-cell sm:px-3 sm:py-2.5">
        <MobileFieldLabel>地址</MobileFieldLabel>
        <span className="flex min-w-0 items-center gap-1.5 text-[11px] text-secondary sm:block sm:text-[12px]">
          <Network size={12} className="shrink-0 text-tertiary sm:hidden" aria-hidden="true" />
          <span className="min-w-0 break-words [overflow-wrap:anywhere]">{endpoint}</span>
        </span>
      </td>
      <td className="col-start-1 row-start-3 min-w-0 pt-2.5 align-middle sm:table-cell sm:px-3 sm:py-2.5">
        <MobileFieldLabel>状态</MobileFieldLabel>
        <div className="flex flex-wrap items-center gap-2">
          <ProxyStatus enabled={proxy.enabled} />
          {proxy.builtIn ? <span className="text-[11px] text-tertiary">内置</span> : null}
        </div>
      </td>
      <td className="col-start-2 row-start-3 min-w-0 pt-2.5 text-right align-middle sm:table-cell sm:px-3 sm:py-2.5 sm:text-left sm:text-secondary">
        <MobileFieldLabel>认证</MobileFieldLabel>
        <span className="inline-flex min-w-0 items-center justify-end gap-1.5 text-[11px] text-tertiary sm:hidden">
          <LockKeyhole size={11} className="shrink-0" aria-hidden="true" />
          <span className="min-w-0 break-words [overflow-wrap:anywhere]">{authentication}</span>
        </span>
        <span className="hidden break-words text-secondary [overflow-wrap:anywhere] sm:inline">
          {proxy.builtIn ? "—" : proxy.passwordConfigured ? proxy.username ?? "已配置" : "无"}
        </span>
      </td>
      <td className="col-start-1 row-start-4 min-w-0 pt-2 align-middle sm:table-cell sm:w-[178px] sm:min-w-[178px] sm:max-w-[178px] sm:px-3 sm:py-2.5">
        <MobileFieldLabel>连通性</MobileFieldLabel>
        <div className="flex min-w-0 items-center gap-1.5 sm:block">
          <Activity size={12} className="shrink-0 text-tertiary sm:hidden" aria-hidden="true" />
          <ProxyTestStatus testing={testing} result={testResult} error={testError} />
        </div>
      </td>
      <td className="col-start-2 row-start-4 pt-1 align-middle sm:table-cell sm:py-2.5 sm:pl-3 sm:pr-0">
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
              label={`将 ${proxy.name} 设为 OAuth 全局出口`}
              title={`将 ${proxy.name} 设为 OAuth 全局出口`}
              className={MOBILE_ICON_ACTION}
              disabled={pending}
              onClick={() => onSetGlobal(proxy)}
            >
              <Globe size={13} />
              <span className="sr-only sm:not-sr-only">OAuth</span>
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
  return <span className="sr-only sm:hidden">{children}</span>;
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
      className="flex h-7 w-auto shrink-0 items-center gap-1.5 text-[11px] font-medium sm:grid sm:h-6 sm:w-[154px] sm:grid-cols-[64px_84px]"
      role={error ? "alert" : "status"}
      aria-label={diagnostic}
      title={diagnostic}
      data-testid="proxy-test-status"
    >
      <span className={cn("sm:text-center", proxyTestToneClass(tone))}>{status}</span>
      <span
        className={cn(
          "text-tertiary tabular-nums sm:text-center",
          latency === "—" && "hidden sm:block",
        )}
      >
        {latency}
      </span>
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

/** Active OAuth default marker, visible next to the name for quick scan. */
function GlobalRouteBadge() {
  return (
    <span
      className="inline-flex shrink-0 items-center gap-1 text-[11px] font-medium text-accent-copy"
      title="选择“跟随 OAuth 全局出口”的账号使用此出口"
    >
      <Globe size={11} strokeWidth={2.25} aria-hidden="true" />
      OAuth 全局出口
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
        "inline-flex items-center rounded-full bg-surface/70 px-1.5 py-0.5 text-[10px] font-medium text-secondary sm:rounded-md sm:bg-surface-muted sm:text-[11px]",
        tone === "success" && "bg-success/10 text-success",
        tone === "neutral" && "text-secondary",
      )}
    >
      {children}
    </span>
  );
}
