import { memo, useEffect, useState } from "react";

import type { ActiveRequestLog } from "../api/request-log-contracts";
import {
  formatDurationMs,
  formatLogListTime,
  processingTone,
  upstreamKindTone,
  upstreamSource,
} from "../model/request-log-presentation";
import { RequestModeBadges } from "./RequestModeBadges";
import { RequestAttemptMarker } from "./RequestAttemptMarker";
import { RequestLogTableCell as Cell } from "./RequestLogTableRow";
import { cn } from "@/shared/lib/cn";

interface ActiveRequestLogRowProps {
  log: ActiveRequestLog;
}

export const ActiveRequestLogCard = memo(function ActiveRequestLogCard({
  log,
}: ActiveRequestLogRowProps) {
  const model = log.publicModel?.trim() || "未解析模型";
  return (
    <article className="log-entry-processing relative min-h-[4.5rem] min-w-0 rounded-[8px] bg-accent/5 px-3 py-2.5">
      <RequestAttemptMarker attemptCount={log.attemptCount} />
      <div className="relative z-10 flex min-w-0 items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-[13px] font-semibold text-primary" title={model}>
          {model}
        </span>
        <StatusBadge />
      </div>
      <div className="relative z-10 mt-1 flex flex-wrap items-center gap-x-2 gap-y-1">
        <time
          className="shrink-0 text-xs tabular-nums text-secondary"
          dateTime={new Date(log.startedAtMs).toISOString()}
        >
          {formatLogListTime(log.startedAtMs)}
        </time>
        <RequestModeBadges isStream={log.isStream} requestedSpeedTier={log.requestedSpeedTier} thinkingLevel={log.thinkingLevel} />
      </div>
      <div className="relative z-10 mt-2 text-xs tabular-nums text-secondary">
        耗时 <RequestElapsed startedAtMs={log.startedAtMs} />
      </div>
      <p className="relative z-10 mt-1.5 break-all border-t border-subtle/60 pt-2 text-xs text-secondary">{upstreamSource(log).displayName}</p>
    </article>
  );
});

export const ActiveRequestLogTableCells = memo(function ActiveRequestLogTableCells({
  log,
}: ActiveRequestLogRowProps) {
  const source = upstreamSource(log);
  const model = log.publicModel?.trim() || "未解析模型";
  return (
    <>
      <Cell className="tabular-nums text-secondary">{formatLogListTime(log.startedAtMs)}</Cell>
      <Cell
        className="font-mono tabular-nums text-secondary"
        title={log.clientIp}
      >
        {log.clientIp}
      </Cell>
      <Cell>
        {source.kind === "none" ? (
          <span className="text-tertiary">未选上游</span>
        ) : (
          <span className={cn("inline-flex max-w-full truncate rounded-full px-1.5 py-0.5 text-xs font-medium", upstreamKindTone(source.kind))}>
            {source.displayName}
          </span>
        )}
      </Cell>
      <Cell className="space-y-0.5" title={model}>
        <span className="block truncate font-medium leading-4 text-primary">{model}</span>
        <RequestModeBadges isStream={log.isStream} requestedSpeedTier={log.requestedSpeedTier} thinkingLevel={log.thinkingLevel} />
      </Cell>
      <Cell className="tabular-nums text-secondary">—</Cell>
      <Cell><StatusBadge /></Cell>
      <Cell className="tabular-nums text-secondary"><RequestElapsed startedAtMs={log.startedAtMs} /></Cell>
      <Cell className="tabular-nums text-secondary">—</Cell>
      <Cell className="text-secondary">—</Cell>
      <Cell className="text-secondary">—</Cell>
      <Cell className="text-secondary">—</Cell>
      <Cell className="text-secondary">—</Cell>
    </>
  );
});

function StatusBadge() {
  return (
    <span className={cn("inline-flex shrink-0 rounded-full px-2 py-0.5 text-xs font-medium", processingTone())}>
      请求中
    </span>
  );
}

function RequestElapsed({ startedAtMs }: { startedAtMs: number }) {
  const [nowMs, setNowMs] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setNowMs(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, []);
  return formatDurationMs(Math.max(0, nowMs - startedAtMs));
}
