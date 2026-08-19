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
import { RequestModeBadges } from "./RequestModeBadges";
import { cn } from "@/shared/lib/cn";

export const REQUEST_LOG_ROW_HEIGHT = 44;
export const requestLogGridClass =
  "grid w-full items-center gap-x-2 px-2 " +
  "[grid-template-columns:minmax(7.5rem,1.25fr)_minmax(6rem,0.85fr)_minmax(8rem,1.5fr)_minmax(9rem,1fr)_minmax(3.5rem,0.55fr)_minmax(4.5rem,0.7fr)_minmax(4.5rem,0.7fr)_minmax(4.5rem,0.7fr)_minmax(4.5rem,0.7fr)_minmax(5.5rem,0.85fr)_minmax(4.5rem,0.7fr)_minmax(3.5rem,0.55fr)]";

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
    <div>
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
        <span className="flex min-w-0 flex-1 items-center gap-1.5">
          <span className="min-w-0 flex-1 truncate text-[13px] font-semibold text-primary">
            {model}
          </span>
          <RequestModeBadges
            isStream={log.isStream}
            requestedSpeedTier={log.requestedSpeedTier}
            effectiveSpeedTier={log.effectiveSpeedTier}
          />
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

export const RequestLogTableCells = memo(function RequestLogTableCells({
  log,
}: {
  log: RequestLog;
}) {
  const source = upstreamSource(log);
  const model = log.publicModel?.trim() || "未解析模型";
  const success = isSuccessOutcome(log.outcome);
  return (
    <>
      <RequestLogTableCell className="tabular-nums text-secondary">
        <time className="block truncate" dateTime={new Date(log.startedAtMs).toISOString()}>
          {formatLogTime(log.startedAtMs)}
        </time>
      </RequestLogTableCell>
      <RequestLogTableCell className="tabular-nums text-secondary" title={log.clientIp}>{log.clientIp}</RequestLogTableCell>
      <RequestLogTableCell>
        {source.kind === "none" ? (
          <span className="text-tertiary">未选上游</span>
        ) : (
          <span className={cn("inline-flex max-w-full truncate rounded-full px-1.5 py-0.5 text-[11px] font-medium", upstreamKindTone(source.kind))}>
            {source.displayName}
          </span>
        )}
      </RequestLogTableCell>
      <RequestLogTableCell className="flex items-center gap-1.5 font-medium text-primary" title={model}>
        <span className="min-w-0 flex-1 truncate">{model}</span>
        <RequestModeBadges
          isStream={log.isStream}
          requestedSpeedTier={log.requestedSpeedTier}
          effectiveSpeedTier={log.effectiveSpeedTier}
        />
      </RequestLogTableCell>
      <RequestLogTableCell>{log.thinkingLevel ?? "-"}</RequestLogTableCell>
      <RequestLogTableCell><ResultBadge log={log} /></RequestLogTableCell>
      <Metric value={formatDurationMs(log.latencyMs)} />
      <Metric value={success ? formatDurationMs(log.firstTokenMs) : "-"} />
      <Metric value={success ? formatTokenCount(log.inputTokens) : "-"} />
      <Metric value={success ? formatTokenCount(log.cacheReadTokens) : "-"} />
      <Metric value={success ? formatTokenCount(log.outputTokens) : "-"} />
      <Metric value={success ? formatTps(outputTps(log)) : "-"} />
    </>
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
  return <RequestLogTableCell className="tabular-nums text-secondary">{value}</RequestLogTableCell>;
}

export function RequestLogTableCell({
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
