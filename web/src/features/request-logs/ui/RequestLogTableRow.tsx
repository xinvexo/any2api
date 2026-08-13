import { ChevronRight } from "lucide-react";

import type { RequestLog } from "../api/request-log-contracts";
import {
  formatDurationMs,
  formatLatencyPair,
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
import { RequestLogExpandedPanel } from "./RequestLogExpandedPanel";
import { cn } from "@/shared/lib/cn";

/**
 * Full-width balanced grid shared by header and body rows.
 * Every column takes a share of free space — no dead right gutter.
 */
export const requestLogGridClass =
  "grid w-full items-center gap-x-2 px-1 " +
  "[grid-template-columns:minmax(0,1.5fr)_minmax(0,0.95fr)_minmax(0,1.15fr)_minmax(0,1fr)_minmax(0,0.55fr)_minmax(0,0.75fr)_minmax(0,0.8fr)_minmax(0,0.8fr)_minmax(0,0.95fr)_minmax(0,0.95fr)_minmax(0,1.15fr)_minmax(0,0.75fr)]";

const cell = "min-w-0 px-1 py-2.5 text-left text-[12px]";
const numCell = `${cell} tabular-nums text-secondary`;

export interface RequestLogRowProps {
  log: RequestLog;
  expanded: boolean;
  onToggle: () => void;
}

/** Mobile: borderless card. */
export function RequestLogCard({ log, expanded, onToggle }: RequestLogRowProps) {
  const panelId = `request-log-card-${log.requestId}`;
  const source = upstreamSource(log);
  const model = log.publicModel?.trim() || null;
  const success = isSuccessOutcome(log.outcome);

  return (
    <article
      className={cn(
        "min-w-0 overflow-hidden rounded-[14px] bg-surface-muted/45",
        expanded && "bg-surface-muted/60",
      )}
    >
      <button
        type="button"
        className="focus-ring flex w-full min-w-0 items-start gap-1.5 px-3 pb-2.5 pt-2.5 text-left"
        aria-expanded={expanded}
        aria-controls={panelId}
        aria-label={expanded ? `收起 ${model ?? log.requestId}` : `展开 ${model ?? log.requestId}`}
        onClick={onToggle}
      >
        <ChevronRight
          size={14}
          className={cn(
            "mt-0.5 shrink-0 text-tertiary transition-transform duration-150",
            expanded && "rotate-90",
          )}
          aria-hidden="true"
        />
        <div className="min-w-0 flex-1 space-y-1">
          <p className="flex flex-wrap gap-x-2 text-[11px] tabular-nums text-tertiary">
            <time dateTime={new Date(log.startedAtMs).toISOString()}>
              {formatLogTime(log.startedAtMs)}
            </time>
            <span className="min-w-0 max-w-full break-all" title={`客户端 IP ${log.clientIp}`}>
              IP {log.clientIp}
            </span>
          </p>
          <div className="flex min-w-0 flex-wrap items-center gap-1.5 text-[11px]">
            <UpstreamSourceInline source={source} />
          </div>
          <div className="flex min-w-0 items-center gap-1.5">
            <span className="min-w-0 truncate text-[13px] font-semibold text-primary">
              {model ?? "未解析模型"}
            </span>
            <ThinkingLevel value={log.thinkingLevel} />
            <span
              className={cn(
                "inline-flex shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium",
                resultTone(log.outcome, log.statusCode),
              )}
              title={`HTTP ${log.statusCode}`}
            >
              {resultBadgeLabel(log.outcome, log.statusCode)}
            </span>
          </div>
          <div className="flex flex-wrap gap-x-3 gap-y-0.5 text-[11px] text-tertiary">
            <span>
              耗时{" "}
              <span className="font-medium text-secondary">
                {success
                  ? formatLatencyPair(log.firstTokenMs, log.latencyMs)
                  : formatDurationMs(log.latencyMs)}
              </span>
            </span>
            {success ? (
              <>
                <TokenMetric label="输入 Token" value={formatTokenCount(log.inputTokens)} />
                <TokenMetric label="输出 Token" value={formatTokenCount(log.outputTokens)} />
                <TokenMetric
                  label="缓存命中 Token"
                  value={formatTokenCount(log.cacheReadTokens)}
                />
                <TokenMetric label="TPS" value={formatTps(outputTps(log))} />
              </>
            ) : log.errorMessage ? (
              <span className="min-w-0 truncate text-danger" title={log.errorMessage}>
                {log.errorMessage}
              </span>
            ) : null}
          </div>
        </div>
      </button>
      {expanded ? (
        <div id={panelId} className="border-t border-subtle/40 pb-3 pl-8 pr-3 pt-2.5">
          <RequestLogExpandedPanel
            requestId={log.requestId}
            outcome={log.outcome}
            attemptCount={log.attemptCount}
          />
        </div>
      ) : null}
    </article>
  );
}

/** Desktop: grid row + optional full-width detail. */
export function RequestLogTableRows({ log, expanded, onToggle }: RequestLogRowProps) {
  const panelId = `request-log-table-${log.requestId}`;
  const source = upstreamSource(log);
  const model = log.publicModel?.trim() || null;
  const success = isSuccessOutcome(log.outcome);

  return (
    <div role="rowgroup">
      <div
        role="row"
        className={cn(
          requestLogGridClass,
          "focus-ring cursor-pointer border-b border-subtle/50 transition-colors outline-none",
          expanded ? "bg-surface-muted/30" : "hover:bg-surface-muted/20",
        )}
        tabIndex={0}
        aria-expanded={expanded}
        aria-controls={panelId}
        aria-label={
          expanded ? `收起 ${model ?? log.requestId}` : `展开 ${model ?? log.requestId}`
        }
        onClick={onToggle}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onToggle();
          }
        }}
      >
        <div role="cell" className={`${cell} tabular-nums text-secondary`}>
          <div className="flex min-w-0 items-center gap-0.5">
            <span className="grid size-5 shrink-0 place-items-center text-tertiary" aria-hidden="true">
              <ChevronRight
                size={13}
                className={cn(
                  "transition-transform duration-150",
                  expanded && "rotate-90",
                )}
              />
            </span>
            <time className="truncate" dateTime={new Date(log.startedAtMs).toISOString()}>
              {formatLogTime(log.startedAtMs)}
            </time>
          </div>
        </div>
        <div role="cell" className={`${cell} tabular-nums text-secondary`}>
          <span className="block truncate" title={log.clientIp}>
            {log.clientIp}
          </span>
        </div>
        <div role="cell" className={cell}>
          <UpstreamSourceInline source={source} />
        </div>
        <div role="cell" className={cell}>
          <span
            className="block truncate font-medium text-primary"
            title={model ?? undefined}
          >
            {model ?? "未解析模型"}
          </span>
        </div>
        <div role="cell" className={cell}>
          <ThinkingLevel value={log.thinkingLevel} />
        </div>
        <div role="cell" className={cell}>
          <span
            className={cn(
              "inline-flex max-w-full truncate rounded-full px-2 py-0.5 text-[11px] font-medium",
              resultTone(log.outcome, log.statusCode),
            )}
            title={`HTTP ${log.statusCode}`}
          >
            {resultBadgeLabel(log.outcome, log.statusCode)}
          </span>
        </div>
        <div role="cell" className={numCell}>
          <span className="block truncate">
            {success ? formatDurationMs(log.firstTokenMs) : "—"}
          </span>
        </div>
        <div role="cell" className={numCell}>
          <span className="block truncate">{formatDurationMs(log.latencyMs)}</span>
        </div>
        <div role="cell" className={numCell}>
          <span className="block truncate">
            {success ? formatTokenCount(log.inputTokens) : "—"}
          </span>
        </div>
        <div role="cell" className={numCell}>
          <span className="block truncate">
            {success ? formatTokenCount(log.outputTokens) : "—"}
          </span>
        </div>
        <div role="cell" className={numCell}>
          <span className="block truncate">
            {success ? formatTokenCount(log.cacheReadTokens) : "—"}
          </span>
        </div>
        <div role="cell" className={numCell}>
          <span className="block truncate">
            {success ? formatTps(outputTps(log)) : "—"}
          </span>
        </div>
      </div>
      {expanded ? (
        <div
          id={panelId}
          role="row"
          className="border-b border-subtle/50 bg-surface-muted/20"
        >
          <div role="cell" className="pb-3 pl-[1.875rem] pr-3 pt-2.5">
            <RequestLogExpandedPanel
              requestId={log.requestId}
              outcome={log.outcome}
              attemptCount={log.attemptCount}
            />
          </div>
        </div>
      ) : null}
    </div>
  );
}

function TokenMetric({ label, value }: { label: string; value: string }) {
  return (
    <span>
      {label}{" "}
      <span className="font-medium text-secondary">{value}</span>
    </span>
  );
}

function UpstreamSourceInline({
  source,
}: {
  source: ReturnType<typeof upstreamSource>;
}) {
  if (source.kind === "none") {
    return <span className="text-[11px] text-tertiary">未选上游</span>;
  }
  const title = [
    source.kindLabel,
    source.displayName,
    source.id ? `(${source.id})` : null,
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <span
      className={cn(
        "inline-flex max-w-full truncate rounded-full px-1.5 py-0.5 text-[11px] font-medium",
        upstreamKindTone(source.kind),
      )}
      title={title}
    >
      {source.displayName}
    </span>
  );
}

function ThinkingLevel({ value }: { value: string | null }) {
  if (!value) {
    return <span className="text-[11px] text-tertiary">—</span>;
  }
  return (
    <span
      className="inline-flex max-w-full truncate rounded-full bg-surface-muted px-1.5 py-0.5 text-[10px] font-medium text-secondary"
      title={value}
    >
      {value}
    </span>
  );
}
