import type { SystemLog } from "../api/system-log-contracts";
import { SystemLogVirtualTable } from "./SystemLogVirtualTable";
import {
  formatBytes,
  formatDuration,
  formatSystemLogTime,
  outcomeLabel,
  statusTone,
} from "../model/system-log-presentation";
import { cn } from "@/shared/lib/cn";

export function SystemLogList({ items }: { items: SystemLog[] }) {
  return (
    <>
      <div
        className="h-full space-y-2 overflow-y-auto pr-1 md:hidden [scrollbar-gutter:stable]"
        role="list"
        aria-label="系统日志列表"
      >
        {items.map((log) => (
          <article
            key={log.requestId}
            role="listitem"
            className="rounded-[6px] border border-subtle bg-surface-muted/40 p-3"
          >
            <div className="flex min-w-0 items-start justify-between gap-3">
              <div className="min-w-0">
                <p className="text-[11px] tabular-nums text-tertiary">
                  {formatSystemLogTime(log.startedAtMs)}
                </p>
                <p className="mt-1 break-all font-mono text-[12px] leading-5 text-primary">
                  <span className="mr-2 font-semibold">{log.method}</span>
                  {log.path}
                </p>
              </div>
              <span className={cn("shrink-0 font-mono text-[13px] font-semibold", statusTone(log))}>
                {log.statusCode ?? "-"}
              </span>
            </div>
            <dl className="mt-3 grid grid-cols-2 gap-x-3 gap-y-2 text-[11px]">
              <Detail label="客户端" value={log.clientIp ?? "未知"} />
              <Detail label="协议" value={log.httpVersion} />
              <Detail label="耗时" value={formatDuration(log.durationMs)} />
              <Detail label="响应" value={formatBytes(log.responseBytes)} />
              <Detail label="结果" value={outcomeLabel(log.outcome)} />
              <Detail label="配置版本" value={String(log.configRevision)} />
            </dl>
          </article>
        ))}
      </div>
      <SystemLogVirtualTable items={items} />
    </>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <dt className="text-tertiary">{label}</dt>
      <dd className="mt-0.5 break-all font-mono text-secondary">{value}</dd>
    </div>
  );
}
