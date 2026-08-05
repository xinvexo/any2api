import { ArrowDownToLine, Clock3, Eye, Monitor, Network } from "lucide-react";
import type { ReactNode } from "react";

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
import { IconButton } from "@/shared/ui/IconButton";

export function SystemLogList({
  items,
  onSelect,
}: {
  items: SystemLog[];
  onSelect: (requestId: string) => void;
}) {
  return (
    <>
      <div
        className="management-scroll-viewport space-y-2 md:hidden"
        role="list"
        aria-label="系统日志列表"
      >
        {items.map((log) => (
          <article
            key={log.requestId}
            role="listitem"
            data-responsive-row="card"
            className="relative rounded-[14px] bg-surface-muted/55 p-3 transition-colors"
          >
            <div className="flex min-w-0 items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <span className="shrink-0 rounded-[6px] bg-surface/70 px-1.5 py-0.5 font-mono text-[10px] font-semibold text-secondary">
                  {log.method}
                </span>
                <time
                  dateTime={new Date(log.startedAtMs).toISOString()}
                  className="min-w-0 truncate text-[11px] tabular-nums text-tertiary"
                >
                  {formatSystemLogTime(log.startedAtMs)}
                </time>
              </div>
              <div className="flex shrink-0 items-center gap-1">
                <span
                  aria-label={`HTTP 状态 ${log.statusCode ?? "未知"}，结果${outcomeLabel(log.outcome)}`}
                  className={cn(
                    "inline-flex shrink-0 items-center rounded-full px-2 py-0.5 text-[11px]",
                    statusTone(log),
                    statusSurfaceTone(log),
                  )}
                >
                  <span className="font-mono font-semibold">{log.statusCode ?? "-"}</span>
                  <span className="mx-1 opacity-45" aria-hidden="true">·</span>
                  <span className="text-[10px] font-medium">{outcomeLabel(log.outcome)}</span>
                </span>
                <IconButton
                  label={`查看完整请求 ${log.uri}`}
                  onClick={() => onSelect(log.requestId)}
                >
                  <Eye size={14} aria-hidden="true" />
                </IconButton>
              </div>
            </div>
            <p className="mt-2 break-words font-mono text-[12px] leading-[1.45] text-primary [overflow-wrap:anywhere]">
              {log.uri}
            </p>
            <dl className="mt-2.5 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[11px] text-secondary">
              <Metadata
                icon={<Monitor size={11} aria-hidden="true" />}
                label="客户端"
                value={log.clientIp ?? "未知"}
                mono
              />
              <Metadata
                icon={<Network size={11} aria-hidden="true" />}
                label="协议"
                value={log.httpVersion}
                mono
              />
              <Metadata
                icon={<Clock3 size={11} aria-hidden="true" />}
                label="耗时"
                value={formatDuration(log.durationMs)}
              />
              <Metadata
                icon={<ArrowDownToLine size={11} aria-hidden="true" />}
                label="响应"
                value={formatBytes(log.responseBytes)}
              />
            </dl>
          </article>
        ))}
      </div>
      <SystemLogVirtualTable items={items} onSelect={onSelect} />
    </>
  );
}

function Metadata({
  icon,
  label,
  value,
  mono = false,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="min-w-0">
      <dt className="sr-only">{label}</dt>
      <dd
        className={cn(
          "flex min-w-0 items-center gap-1 text-secondary [&_svg]:shrink-0 [&_svg]:text-tertiary",
          mono && "font-mono",
        )}
      >
        {icon}
        <span className="min-w-0 break-all tabular-nums">{value}</span>
      </dd>
    </div>
  );
}

function statusSurfaceTone(log: SystemLog) {
  switch (statusTone(log)) {
    case "text-success":
      return "bg-success/10";
    case "text-warning":
      return "bg-warning/10";
    case "text-danger":
      return "bg-danger/10";
    default:
      return "bg-surface/70";
  }
}
