import { ChevronRight } from "lucide-react";

import type { RequestLog } from "../api/request-log-contracts";
import {
  formatDurationMs,
  formatLogTime,
  formatTokenCount,
  resultLabel,
  statusTone,
  totalTokens,
  upstreamKindTone,
  upstreamSource,
} from "../model/request-log-presentation";
import { RequestLogExpandedPanel } from "./RequestLogExpandedPanel";
import { cn } from "@/shared/lib/cn";

export interface RequestLogRowProps {
  log: RequestLog;
  expanded: boolean;
  onToggle: () => void;
}

/** Mobile: borderless card. */
export function RequestLogCard({ log, expanded, onToggle }: RequestLogRowProps) {
  const panelId = `request-log-card-${log.requestId}`;
  const source = upstreamSource(log);
  const tokens = totalTokens(log);
  const model = log.publicModel?.trim() || null;

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
          <div className="flex min-w-0 items-center gap-1.5">
            <span className="min-w-0 truncate text-[13px] font-semibold text-primary">
              {model ?? "未解析模型"}
            </span>
            <span
              className={cn(
                "inline-flex shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium",
                statusTone(log.statusCode),
              )}
            >
              {resultLabel(log.statusCode)}
            </span>
          </div>
          <p className="text-[11px] tabular-nums text-tertiary">
            <time dateTime={new Date(log.startedAtMs).toISOString()}>
              {formatLogTime(log.startedAtMs)}
            </time>
          </p>
          <div className="flex min-w-0 flex-wrap items-center gap-1.5 text-[11px]">
            <span className="text-tertiary">来源</span>
            <UpstreamSourceInline source={source} />
          </div>
          <div className="flex flex-wrap gap-x-3 gap-y-0.5 text-[11px] text-tertiary">
            <span>
              首字{" "}
              <span className="font-medium text-secondary">
                {formatDurationMs(log.firstTokenMs)}
              </span>
            </span>
            <span>
              延迟{" "}
              <span className="font-medium text-secondary">
                {formatDurationMs(log.latencyMs)}
              </span>
            </span>
            <span>
              Token{" "}
              <span className="font-medium text-secondary">{formatTokenCount(tokens)}</span>
            </span>
            {log.errorMessage ? (
              <span className="min-w-0 truncate text-danger" title={log.errorMessage}>
                {log.errorMessage}
              </span>
            ) : log.errorClass ? (
              <span className="text-warning" title={log.errorClass}>
                {log.errorClass}
              </span>
            ) : null}
          </div>
        </div>
      </button>
      {expanded ? (
        <div id={panelId} className="border-t border-subtle/40 px-3 pb-3 pt-2.5">
          <RequestLogExpandedPanel requestId={log.requestId} summary={log} />
        </div>
      ) : null}
    </article>
  );
}

/** Desktop: data row + optional full-width detail row. */
export function RequestLogTableRows({ log, expanded, onToggle }: RequestLogRowProps) {
  const panelId = `request-log-table-${log.requestId}`;
  const source = upstreamSource(log);
  const tokens = totalTokens(log);
  const model = log.publicModel?.trim() || null;

  return (
    <>
      <tr
        className={cn(
          "border-b border-subtle/50 transition-colors",
          expanded ? "bg-surface-muted/30" : "hover:bg-surface-muted/20",
        )}
      >
        <td className="px-2 py-2.5 align-middle">
          <button
            type="button"
            className="focus-ring flex max-w-full min-w-0 items-center gap-1.5 rounded-[6px] text-left"
            aria-expanded={expanded}
            aria-controls={panelId}
            aria-label={
              expanded ? `收起 ${model ?? log.requestId}` : `展开 ${model ?? log.requestId}`
            }
            onClick={onToggle}
          >
            <ChevronRight
              size={14}
              className={cn(
                "shrink-0 text-tertiary transition-transform duration-150",
                expanded && "rotate-90",
              )}
              aria-hidden="true"
            />
            <span className="min-w-0 truncate text-[12px] font-medium text-primary">
              {model ?? "未解析模型"}
            </span>
          </button>
        </td>
        <td className="whitespace-nowrap px-2 py-2.5 align-middle text-[12px] tabular-nums text-secondary">
          <time dateTime={new Date(log.startedAtMs).toISOString()}>
            {formatLogTime(log.startedAtMs)}
          </time>
        </td>
        <td className="min-w-[9rem] px-2 py-2.5 align-middle">
          <UpstreamSourceInline source={source} />
        </td>
        <td className="px-2 py-2.5 align-middle">
          <span
            className={cn(
              "inline-flex rounded-full px-2 py-0.5 text-[11px] font-medium",
              statusTone(log.statusCode),
            )}
            title={`HTTP ${log.statusCode}`}
          >
            {resultLabel(log.statusCode)}
          </span>
        </td>
        <td className="whitespace-nowrap px-2 py-2.5 align-middle text-[12px] tabular-nums text-secondary">
          {formatDurationMs(log.firstTokenMs)}
        </td>
        <td className="whitespace-nowrap px-2 py-2.5 align-middle text-[12px] tabular-nums text-secondary">
          {formatDurationMs(log.latencyMs)}
        </td>
        <td className="whitespace-nowrap px-2 py-2.5 text-right align-middle text-[12px] tabular-nums text-secondary">
          {formatTokenCount(tokens)}
        </td>
      </tr>
      {expanded ? (
        <tr className="border-b border-subtle/50 bg-surface-muted/20">
          <td id={panelId} colSpan={7} className="px-3 pb-3 pt-2.5">
            <RequestLogExpandedPanel requestId={log.requestId} summary={log} />
          </td>
        </tr>
      ) : null}
    </>
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
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-1.5">
      <span
        className={cn(
          "inline-flex shrink-0 rounded-full px-1.5 py-0.5 text-[10px] font-medium",
          upstreamKindTone(source.kind),
        )}
      >
        {source.kindLabel}
      </span>
      <span
        className="min-w-0 max-w-full truncate font-mono text-[11px] text-primary"
        title={source.id ?? undefined}
      >
        {source.shortId}
      </span>
    </div>
  );
}
