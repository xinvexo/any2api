import type { ActiveRequestLog } from "../api/request-log-contracts";
import {
  formatLogTime,
  processingTone,
  upstreamKindTone,
  upstreamSource,
} from "../model/request-log-presentation";
import { cn } from "@/shared/lib/cn";
import { requestLogGridClass } from "./RequestLogTableRow";

export function ActiveRequestLogCard({ log }: { log: ActiveRequestLog }) {
  const source = upstreamSource(log);
  const model = log.publicModel?.trim() || "未解析模型";
  return (
    <article className="min-w-0 overflow-hidden rounded-[14px] bg-accent/5">
      <div className="flex w-full min-w-0 items-start gap-1.5 px-3 py-2.5 text-left">
        <div className="min-w-0 flex-1 space-y-1">
          <p className="flex flex-wrap gap-x-2 text-[11px] tabular-nums text-tertiary">
            <time dateTime={new Date(log.startedAtMs).toISOString()}>
              {formatLogTime(log.startedAtMs)}
            </time>
            <span className="min-w-0 max-w-full break-all">IP {log.clientIp}</span>
          </p>
          <div className="flex min-w-0 flex-wrap items-center gap-1.5 text-[11px]">
            <UpstreamSourceInline source={source} />
          </div>
          <div className="flex min-w-0 items-center gap-1.5">
            <span className="min-w-0 truncate text-[13px] font-semibold text-primary">{model}</span>
            <StatusBadge />
          </div>
          <p className="text-[11px] text-tertiary">等待请求完成</p>
        </div>
      </div>
    </article>
  );
}

export function ActiveRequestLogTableRow({ log }: { log: ActiveRequestLog }) {
  const source = upstreamSource(log);
  const model = log.publicModel?.trim() || "未解析模型";
  return (
    <div role="row" className={cn(requestLogGridClass, "border-b border-subtle/50 bg-accent/5")}>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px] tabular-nums text-secondary">
        <div className="flex min-w-0 items-center gap-0.5">
          <span className="size-5 shrink-0" aria-hidden="true" />
          <time className="truncate" dateTime={new Date(log.startedAtMs).toISOString()}>
            {formatLogTime(log.startedAtMs)}
          </time>
        </div>
      </div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px] tabular-nums text-secondary">
        <span className="block truncate" title={log.clientIp}>{log.clientIp}</span>
      </div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px]"><UpstreamSourceInline source={source} /></div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px]"><span className="block truncate font-medium text-primary">{model}</span></div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px]"><ThinkingLevel value={log.thinkingLevel} /></div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px]"><StatusBadge /></div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px] tabular-nums text-secondary">—</div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px] tabular-nums text-secondary">—</div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px] tabular-nums text-secondary">—</div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px] tabular-nums text-secondary">—</div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px] tabular-nums text-secondary">—</div>
      <div role="cell" className="min-w-0 px-1 py-2.5 text-left text-[12px] tabular-nums text-secondary">—</div>
    </div>
  );
}

function StatusBadge() {
  return (
    <span className={cn("inline-flex shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium", processingTone())}>
      请求中
    </span>
  );
}

function UpstreamSourceInline({ source }: { source: ReturnType<typeof upstreamSource> }) {
  if (source.kind === "none") {
    return <span className="text-[11px] text-tertiary">未选上游</span>;
  }
  return (
    <span className={cn("inline-flex max-w-full truncate rounded-full px-1.5 py-0.5 text-[11px] font-medium", upstreamKindTone(source.kind))}>
      {source.displayName}
    </span>
  );
}

function ThinkingLevel({ value }: { value: string | null }) {
  return value ? (
    <span className="inline-flex max-w-full truncate rounded-full bg-surface-muted px-1.5 py-0.5 text-[10px] font-medium text-secondary">{value}</span>
  ) : (
    <span className="text-[11px] text-tertiary">—</span>
  );
}
