import { memo } from "react";

import type { RequestLog } from "../api/request-log-contracts";
import {
  formatDurationMs,
  formatLogTime,
  formatTokenCount,
  formatTps,
  isSuccessOutcome,
  outputTps,
  resultBadgeLabel,
  resultTone,
  upstreamKindTone,
  upstreamSource,
} from "../model/request-log-presentation";
import { cn } from "@/shared/lib/cn";

export const REQUEST_LOG_ROW_HEIGHT = 44;
export const requestLogGridClass =
  "grid w-full items-center gap-x-2 px-2 " +
  "[grid-template-columns:minmax(7.5rem,1.25fr)_minmax(6rem,0.85fr)_minmax(8rem,1.5fr)_minmax(7rem,1fr)_minmax(3.5rem,0.55fr)_minmax(4.5rem,0.7fr)_minmax(4.5rem,0.7fr)_minmax(4.5rem,0.7fr)_minmax(4.5rem,0.7fr)_minmax(4.5rem,0.7fr)_minmax(5.5rem,0.85fr)_minmax(3.5rem,0.55fr)]";

interface RequestLogRowProps {
  log: RequestLog;
  selected: boolean;
  onSelect: (requestId: string) => void;
}

export const RequestLogCard = memo(function RequestLogCard({
  log,
  selected,
  onSelect,
}: RequestLogRowProps) {
  const source = upstreamSource(log);
  const model = log.publicModel?.trim() || "未解析模型";
  const success = isSuccessOutcome(log.outcome);
  return (
    <div role="listitem">
      <div
        role="button"
        tabIndex={0}
        className={cn(
          "focus-ring block min-h-[4.5rem] w-full min-w-0 cursor-pointer select-text rounded-[8px] bg-surface-muted/45 px-3 py-2.5 text-left outline-none transition-colors",
          selected ? "bg-accent/10 ring-1 ring-accent/35" : "hover:bg-surface-muted/70",
        )}
        aria-label={`查看请求 ${model}`}
        title="双击查看详情"
        aria-pressed={selected}
        onDoubleClick={() => onSelect(log.requestId)}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onSelect(log.requestId);
          }
        }}
      >
      <div className="flex min-w-0 items-center gap-2">
        <time
          className="shrink-0 text-[11px] tabular-nums text-tertiary"
          dateTime={new Date(log.startedAtMs).toISOString()}
        >
          {formatLogTime(log.startedAtMs)}
        </time>
        <span className="min-w-0 flex-1 truncate text-[13px] font-semibold text-primary">
          {model}
        </span>
        <ResultBadge log={log} />
      </div>
      <div className="mt-1.5 flex min-w-0 items-center gap-2 text-[11px] text-secondary">
        <span className="shrink-0 tabular-nums">
          {formatDurationMs(log.latencyMs)}
        </span>
        <span className="shrink-0 tabular-nums">In {success ? formatTokenCount(log.inputTokens) : "-"}</span>
        <span className="shrink-0 tabular-nums">Out {success ? formatTokenCount(log.outputTokens) : "-"}</span>
        <span className="min-w-0 flex-1 truncate text-right" title={source.displayName}>
          {source.displayName}
        </span>
      </div>
      </div>
    </div>
  );
});

export const RequestLogTableRow = memo(function RequestLogTableRow({
  log,
  selected,
  onSelect,
}: RequestLogRowProps) {
  const source = upstreamSource(log);
  const model = log.publicModel?.trim() || "未解析模型";
  const success = isSuccessOutcome(log.outcome);
  return (
    <div
      role="row"
      tabIndex={0}
      aria-selected={selected}
      aria-label={`查看请求 ${model}`}
      title="双击查看详情"
      className={cn(
        requestLogGridClass,
        "focus-ring relative isolate h-11 cursor-pointer rounded-[8px] border-b border-subtle/50 text-[12px] outline-none before:pointer-events-none before:absolute before:inset-1 before:z-[-1] before:rounded-[8px] before:content-[''] before:transition-colors",
        selected ? "before:bg-accent/10" : "hover:before:bg-surface-muted/45",
      )}
      onDoubleClick={() => onSelect(log.requestId)}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect(log.requestId);
        }
      }}
    >
      <Cell className="tabular-nums text-secondary">
        <time className="block truncate" dateTime={new Date(log.startedAtMs).toISOString()}>
          {formatLogTime(log.startedAtMs)}
        </time>
      </Cell>
      <Cell className="tabular-nums text-secondary" title={log.clientIp}>{log.clientIp}</Cell>
      <Cell>
        {source.kind === "none" ? (
          <span className="text-tertiary">未选上游</span>
        ) : (
          <span className={cn("inline-flex max-w-full truncate rounded-full px-1.5 py-0.5 text-[11px] font-medium", upstreamKindTone(source.kind))}>
            {source.displayName}
          </span>
        )}
      </Cell>
      <Cell className="font-medium text-primary" title={model}>{model}</Cell>
      <Cell>{log.thinkingLevel ?? "-"}</Cell>
      <Cell><ResultBadge log={log} /></Cell>
      <Metric value={success ? formatDurationMs(log.firstTokenMs) : "-"} />
      <Metric value={formatDurationMs(log.latencyMs)} />
      <Metric value={success ? formatTokenCount(log.inputTokens) : "-"} />
      <Metric value={success ? formatTokenCount(log.outputTokens) : "-"} />
      <Metric value={success ? formatTokenCount(log.cacheReadTokens) : "-"} />
      <Metric value={success ? formatTps(outputTps(log)) : "-"} />
    </div>
  );
});

function ResultBadge({ log }: { log: RequestLog }) {
  return (
    <span
      className={cn(
        "inline-flex max-w-full shrink-0 truncate rounded-full px-2 py-0.5 text-[11px] font-medium",
        resultTone(log.outcome, log.statusCode),
      )}
      title={`HTTP ${log.statusCode}`}
    >
      {resultBadgeLabel(log.outcome, log.statusCode)}
    </span>
  );
}

function Metric({ value }: { value: string }) {
  return <Cell className="tabular-nums text-secondary">{value}</Cell>;
}

function Cell({
  children,
  className,
  title,
}: {
  children: React.ReactNode;
  className?: string;
  title?: string;
}) {
  return <div role="cell" title={title} className={cn("min-w-0 truncate px-1 text-left", className)}>{children}</div>;
}
