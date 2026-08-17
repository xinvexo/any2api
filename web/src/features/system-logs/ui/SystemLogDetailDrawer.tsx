import { RefreshCw } from "lucide-react";

import type {
  SystemLogDetail,
  SystemLogHeader,
  SystemLogMessage,
} from "../api/system-log-contracts";
import { formatBytes, formatDuration } from "../model/system-log-presentation";
import { useSystemLog } from "../model/use-system-log";
import { SystemLogBody } from "./SystemLogBody";
import { Button } from "@/shared/ui/Button";
import { SideDrawer } from "@/shared/ui/SideDrawer";

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
      title="完整 HTTP 请求"
      description={log ? `${log.method} ${log.uri}` : "正在读取系统日志详情"}
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
  const { log, exchange } = detail;
  return (
    <div className="min-w-0 space-y-6">
      <dl className="grid grid-cols-2 gap-x-4 gap-y-3 text-[12px] sm:grid-cols-3">
        <Metadata label="Request ID" value={log.requestId} mono />
        <Metadata label="客户端 IP" value={log.clientIp ?? "未知"} mono />
        <Metadata label="HTTP" value={`${log.httpVersion} · ${log.statusCode ?? "未知"}`} mono />
        <Metadata label="耗时" value={formatDuration(log.durationMs)} />
        <Metadata label="响应字节" value={formatBytes(log.responseBytes)} />
      </dl>

      <div>
        <p className="text-[11px] font-medium uppercase tracking-[0.08em] text-tertiary">完整 URI</p>
        <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-all rounded-[10px] bg-surface-muted/70 p-3 font-mono text-[12px] leading-5 text-primary [overflow-wrap:anywhere]">
          {log.uri}
        </pre>
      </div>

      {exchange ? (
        <div className="space-y-7">
          <MessageSection title="请求" message={exchange.request} />
          <MessageSection title="响应" message={exchange.response} />
        </div>
      ) : (
        <p className="border-t border-subtle pt-5 text-[13px] text-secondary">
          这条记录创建于完整交换捕获启用前，没有可恢复的 Header 或 Body。
        </p>
      )}
    </div>
  );
}

function MessageSection({ title, message }: { title: string; message: SystemLogMessage }) {
  return (
    <section className="border-t border-subtle pt-5" aria-label={`${title}详情`}>
      <h3 className="text-[14px] font-semibold">{title}</h3>
      <HeaderList headers={message.headers} />
      <SystemLogBody body={message.body} headers={message.headers} />
    </section>
  );
}

function HeaderList({ headers }: { headers: SystemLogHeader[] }) {
  return (
    <div className="mt-4">
      <p className="text-[11px] font-medium uppercase tracking-[0.08em] text-tertiary">
        Headers · {headers.length}
      </p>
      {headers.length === 0 ? (
        <p className="mt-2 text-[12px] text-secondary">无 Header</p>
      ) : (
        <ul className="mt-2 divide-y divide-subtle overflow-hidden rounded-[10px] bg-surface-muted/55">
          {headers.map((header, index) => (
            <li key={`${header.name}-${index}`} className="grid gap-1 px-3 py-2 sm:grid-cols-[11rem_minmax(0,1fr)] sm:gap-3">
              <code className="break-all text-[11px] font-semibold text-secondary">{header.name}</code>
              <pre className="min-w-0 whitespace-pre-wrap break-all font-mono text-[11px] leading-5 text-primary [overflow-wrap:anywhere]">
                {header.encoding === "base64" ? `[Base64] ${header.value}` : header.value}
              </pre>
            </li>
          ))}
        </ul>
      )}
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
    <div className="space-y-5" aria-busy="true" aria-label="正在读取完整 HTTP 请求">
      <div className="h-16 animate-pulse rounded-[10px] bg-surface-muted" />
      <div className="h-28 animate-pulse rounded-[10px] bg-surface-muted" />
      <div className="h-48 animate-pulse rounded-[10px] bg-surface-muted" />
    </div>
  );
}
