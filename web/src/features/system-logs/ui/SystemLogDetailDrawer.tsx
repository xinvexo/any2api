import { RefreshCw } from "lucide-react";
import { Link } from "react-router-dom";

import type { SystemLogDetail } from "../api/system-log-contracts";
import {
  formatBytes,
  formatDuration,
  formatSystemLogTime,
  outcomeLabel,
} from "../model/system-log-presentation";
import { useSystemLog } from "../model/use-system-log";
import { Button } from "@/shared/ui/Button";
import { SideDrawer } from "@/shared/ui/SideDrawer";
import { buttonClassName } from "@/shared/ui/button-class-name";

export function SystemLogDetailDrawer({
  requestId,
  onClose,
}: {
  requestId: string | null;
  onClose: () => void;
}) {
  const query = useSystemLog(requestId ?? "");
  const log = query.data?.log;

  return (
    <SideDrawer
      open={requestId !== null}
      title="HTTP 访问详情"
      description={log ? `${log.method} ${log.path}` : "正在读取系统日志详情"}
      onClose={onClose}
      wide
    >
      {query.isPending && !query.data ? (
        <DetailSkeleton />
      ) : !query.data ? (
        <div className="space-y-3" role="alert">
          <p className="text-[13px] text-danger">系统日志详情读取失败或已经被清理。</p>
          <Button size="sm" variant="ghost" onClick={() => void query.refetch()}>
            <RefreshCw size={14} />
            重试
          </Button>
        </div>
      ) : (
        <DetailContent detail={query.data} />
      )}
    </SideDrawer>
  );
}

function DetailContent({ detail }: { detail: SystemLogDetail }) {
  const { log } = detail;
  return (
    <div className="min-w-0 space-y-6">
      <dl className="grid grid-cols-2 gap-x-4 gap-y-3 text-[12px] sm:grid-cols-3">
        <Metadata label="Request ID" value={log.requestId} mono />
        <Metadata label="请求时间" value={formatSystemLogTime(log.startedAtMs)} />
        <Metadata label="客户端 IP" value={log.clientIp ?? "未知"} mono />
        <Metadata label="方法" value={log.method} mono />
        <Metadata label="HTTP" value={`${log.httpVersion} · ${log.statusCode ?? "未知"}`} mono />
        <Metadata label="耗时" value={formatDuration(log.durationMs)} />
        <Metadata label="响应字节" value={formatBytes(log.responseBytes)} />
        <Metadata label="结果" value={outcomeLabel(log.outcome)} />
        <Metadata label="配置版本" value={String(log.configRevision)} />
      </dl>

      <div>
        <p className="text-[12px] font-medium text-secondary">完整请求路径</p>
        <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-all rounded-[10px] bg-surface-muted/70 p-3 font-mono text-[12px] leading-5 text-primary [overflow-wrap:anywhere]">
          {log.path}
        </pre>
      </div>
      {detail.hasRequestLog ? <Link className={buttonClassName({ variant: "secondary" })} to={`/logs/${encodeURIComponent(log.requestId)}`}>查看模型请求详情</Link> : null}
    </div>
  );
}

function Metadata({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="min-w-0">
      <dt className="text-tertiary">{label}</dt>
      <dd className={`mt-0.5 break-all text-primary ${mono ? "font-mono" : ""}`}>{value}</dd>
    </div>
  );
}

function DetailSkeleton() {
  return (
    <div className="space-y-5" aria-busy="true" aria-label="正在读取 HTTP 请求元数据">
      <div className="h-16 animate-pulse rounded-[10px] bg-surface-muted" />
      <div className="h-28 animate-pulse rounded-[10px] bg-surface-muted" />
    </div>
  );
}
