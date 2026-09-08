import { ChevronRight } from "lucide-react";
import { memo } from "react";

import type { RequestLog } from "../api/request-log-contracts";
import {
  formatDurationMs,
  formatLogListTime,
  formatTokenCount,
  formatTps,
  isSuccessOutcome,
  outputTps,
  presentRequestQuotaCost,
  resultBadgeLabel,
  resultTone,
  upstreamKindTone,
  upstreamSource,
} from "../model/request-log-presentation";
import { RequestModeBadges } from "./RequestModeBadges";
import { RequestAttemptMarker } from "./RequestAttemptMarker";
import { cn } from "@/shared/lib/cn";
import { RowActionButton } from "@/shared/ui/RowActionButton";

export const REQUEST_LOG_ROW_HEIGHT = 44;
export const requestLogGridClass =
  "grid w-full items-center gap-x-2 px-2 " +
  "[grid-template-columns:7rem_7.5rem_minmax(8rem,1.25fr)_minmax(10rem,1.5fr)_minmax(5.5rem,0.8fr)_minmax(4.5rem,0.7fr)_minmax(4.5rem,0.7fr)_minmax(4.5rem,0.7fr)_minmax(4.5rem,0.7fr)_minmax(5.5rem,0.85fr)_minmax(4.5rem,0.7fr)_minmax(3.5rem,0.55fr)]";

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
  const quotaCost = presentRequestQuotaCost(log.quotaCost);
  return (
    <article
      className={cn(
        "relative w-full min-w-0 select-text rounded-[8px] bg-surface-muted/45 px-3 py-2.5 text-left",
        selected && "bg-accent/10 ring-1 ring-accent/35",
      )}
      aria-label={`请求 ${model}`}
    >
      <RequestAttemptMarker attemptCount={log.attemptCount} />
      <div className="relative z-10 flex min-w-0 items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-[13px] font-semibold text-primary" title={model}>
          {model}
        </span>
        <ResultBadge log={log} />
      </div>
      <div className="relative z-10 mt-1 flex flex-wrap items-center gap-x-2 gap-y-1">
        <time
          className="shrink-0 text-xs tabular-nums text-secondary"
          dateTime={new Date(log.startedAtMs).toISOString()}
        >
          {formatLogListTime(log.startedAtMs)}
        </time>
        <RequestModeBadges
          isStream={log.isStream}
          requestedSpeedTier={log.requestedSpeedTier}
          thinkingLevel={log.thinkingLevel}
        />
      </div>
      <div className="relative z-10 mt-2 grid min-w-0 grid-cols-2 gap-x-3 gap-y-1 text-xs tabular-nums text-secondary">
        <span>
          耗时 {formatDurationMs(log.latencyMs)}
        </span>
        <span className="min-w-0 truncate text-right font-medium text-primary" title={quotaCost?.detail}>
          费用 {quotaCost?.value ?? "—"}
        </span>
        <span>In {success ? formatTokenCount(log.inputTokens) : "—"}</span>
        <span className="text-right">Out {success ? formatTokenCount(log.outputTokens) : "—"}</span>
      </div>
      <div className="relative z-10 mt-1.5 flex min-w-0 items-center gap-2 border-t border-subtle/60 pt-1 text-xs text-secondary">
        <span className="min-w-0 flex-1 break-all" title={source.displayName}>
          {source.displayName}
        </span>
        <RowActionButton label={`查看请求 ${model}`} className="shrink-0" onClick={() => onSelect(log.requestId)}>
          详情
          <ChevronRight size={14} aria-hidden="true" />
        </RowActionButton>
      </div>
    </article>
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
  const quotaCost = presentRequestQuotaCost(log.quotaCost);
  return (
    <>
      <RequestLogTableCell className="tabular-nums text-secondary">
        <time className="block truncate" dateTime={new Date(log.startedAtMs).toISOString()}>
          {formatLogListTime(log.startedAtMs)}
        </time>
      </RequestLogTableCell>
      <RequestLogTableCell
        className="font-mono tabular-nums text-secondary"
        title={log.clientIp}
      >
        {log.clientIp}
      </RequestLogTableCell>
      <RequestLogTableCell>
        {source.kind === "none" ? (
          <span className="text-tertiary">未选上游</span>
        ) : (
          <span className={cn("inline-flex max-w-full truncate rounded-full px-1.5 py-0.5 text-xs font-medium", upstreamKindTone(source.kind))}>
            {source.displayName}
          </span>
        )}
      </RequestLogTableCell>
      <RequestLogTableCell className="space-y-0.5" title={model}>
        <span className="block truncate font-medium leading-4 text-primary">{model}</span>
        <RequestModeBadges isStream={log.isStream} requestedSpeedTier={log.requestedSpeedTier} thinkingLevel={log.thinkingLevel} />
      </RequestLogTableCell>
      <Metric value={quotaCost?.value ?? "—"} title={quotaCost?.detail} />
      <RequestLogTableCell><ResultBadge log={log} /></RequestLogTableCell>
      <Metric value={formatDurationMs(log.latencyMs)} />
      <Metric value={success ? formatDurationMs(log.firstTokenMs) : "—"} />
      <Metric value={success ? formatTokenCount(log.inputTokens) : "—"} />
      <Metric value={success ? formatTokenCount(log.cacheReadTokens) : "—"} />
      <Metric value={success ? formatTokenCount(log.outputTokens) : "—"} />
      <Metric value={success ? formatTps(outputTps(log)) : "—"} />
    </>
  );
});

function ResultBadge({ log }: { log: RequestLog }) {
  return (
    <span
      className={cn(
        "inline-flex max-w-full shrink-0 truncate rounded-full px-2 py-0.5 text-xs font-medium",
        resultTone(log.outcome, log.statusCode),
      )}
      title={`HTTP ${log.statusCode}`}
    >
      {resultBadgeLabel(log.outcome, log.statusCode)}
    </span>
  );
}

function Metric({ value, title }: { value: string; title?: string }) {
  return <RequestLogTableCell className="tabular-nums text-secondary" title={title}>{value}</RequestLogTableCell>;
}

export function RequestLogTableCell({
  children,
  className,
  title,
  truncate = true,
}: {
  children: React.ReactNode;
  className?: string;
  title?: string;
  truncate?: boolean;
}) {
  return <div role="cell" title={title} className={cn("relative z-10 min-w-0 px-1 text-left", truncate && "truncate", className)}>{children}</div>;
}
