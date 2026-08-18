import { memo } from "react";

import type { ActiveRequestLog } from "../api/request-log-contracts";
import {
  formatDurationMs,
  formatLogTime,
  processingTone,
  upstreamKindTone,
  upstreamSource,
} from "../model/request-log-presentation";
import { RequestLogTableCell as Cell } from "./RequestLogTableRow";
import { cn } from "@/shared/lib/cn";

interface ActiveRequestLogRowProps {
  log: ActiveRequestLog;
  nowMs: number;
}

export const ActiveRequestLogCard = memo(function ActiveRequestLogCard({
  log,
  nowMs,
}: ActiveRequestLogRowProps) {
  const model = log.publicModel?.trim() || "未解析模型";
  return (
    <article className="log-entry-processing min-h-[4.5rem] min-w-0 rounded-[8px] bg-accent/5 px-3 py-2.5">
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
        <StatusBadge />
      </div>
      <div className="mt-1.5 flex min-w-0 items-center gap-2 text-[11px] text-secondary">
        <span className="shrink-0 tabular-nums">{elapsed(log, nowMs)}</span>
        <span className="min-w-0 flex-1 truncate text-right">
          {upstreamSource(log).displayName}
        </span>
      </div>
    </article>
  );
});

export const ActiveRequestLogTableCells = memo(function ActiveRequestLogTableCells({
  log,
  nowMs,
}: ActiveRequestLogRowProps) {
  const source = upstreamSource(log);
  const model = log.publicModel?.trim() || "未解析模型";
  return (
    <>
      <Cell className="tabular-nums text-secondary">{formatLogTime(log.startedAtMs)}</Cell>
      <Cell className="tabular-nums text-secondary">{log.clientIp}</Cell>
      <Cell>
        {source.kind === "none" ? (
          <span className="text-tertiary">未选上游</span>
        ) : (
          <span className={cn("inline-flex max-w-full truncate rounded-full px-1.5 py-0.5 text-[11px] font-medium", upstreamKindTone(source.kind))}>
            {source.displayName}
          </span>
        )}
      </Cell>
      <Cell className="font-medium text-primary">{model}</Cell>
      <Cell>{log.thinkingLevel ?? "-"}</Cell>
      <Cell><StatusBadge /></Cell>
      <Cell className="tabular-nums text-secondary">{elapsed(log, nowMs)}</Cell>
      <Cell className="tabular-nums text-secondary">-</Cell>
      <Cell className="text-secondary">-</Cell>
      <Cell className="text-secondary">-</Cell>
      <Cell className="text-secondary">-</Cell>
      <Cell className="text-secondary">-</Cell>
    </>
  );
});

function StatusBadge() {
  return (
    <span className={cn("inline-flex shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium", processingTone())}>
      请求中
    </span>
  );
}

function elapsed(log: ActiveRequestLog, nowMs: number) {
  return formatDurationMs(Math.max(0, nowMs - log.startedAtMs));
}
