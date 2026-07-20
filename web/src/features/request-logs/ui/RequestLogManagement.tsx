import { RefreshCw, ScrollText } from "lucide-react";
import { Link } from "react-router-dom";

import type { RequestLog } from "../api/request-log-contracts";
import { getRequestLogErrorMessage } from "../model/request-log-error";
import { useRequestLogs } from "../model/use-request-logs";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function RequestLogManagement() {
  const query = useRequestLogs();

  if (query.isPending && !query.data) {
    return (
      <Surface
        className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary"
        aria-busy="true"
      >
        正在读取请求日志
      </Surface>
    );
  }
  if (!query.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取请求日志</p>
        <p className="mt-2 text-sm text-secondary">{getRequestLogErrorMessage(query.error)}</p>
        <Button className="mt-5" onClick={() => void query.refetch()} disabled={query.isFetching}>
          <RefreshCw size={15} />
          重试
        </Button>
      </Surface>
    );
  }

  return (
    <div className="space-y-5" aria-busy={query.isFetching}>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <p className="text-sm text-secondary">
          最近 {query.data.items.length} 条
          <span className="mx-2 text-tertiary">·</span>
          队列 {query.data.telemetry.queuedRecords}
          <span className="mx-2 text-tertiary">·</span>
          丢弃 {query.data.telemetry.droppedRecords}
        </p>
        <Button variant="ghost" onClick={() => void query.refetch()} disabled={query.isFetching}>
          <RefreshCw size={15} className={query.isFetching ? "animate-spin" : undefined} />
          刷新
        </Button>
      </div>

      {query.isError ? (
        <Surface className="border-warning/40 p-4 text-sm text-secondary" role="status">
          刷新失败，当前仍显示最近一次有效数据：{getRequestLogErrorMessage(query.error)}
        </Surface>
      ) : null}

      <Surface className="overflow-hidden">
        {query.data.items.length === 0 ? (
          <div className="flex min-h-56 flex-col items-center justify-center px-6 text-center">
            <ScrollText size={24} className="text-tertiary" aria-hidden="true" />
            <p className="mt-4 font-semibold">还没有请求日志</p>
            <p className="mt-2 text-sm text-secondary">
              通过网关完成一次 Codex 或 Claude 请求后，记录会出现在这里。
            </p>
          </div>
        ) : (
          <div className="divide-y divide-subtle">
            {query.data.items.map((log) => (
              <RequestLogRow key={log.requestId} log={log} />
            ))}
          </div>
        )}
      </Surface>
    </div>
  );
}

function RequestLogRow({ log }: { log: RequestLog }) {
  return (
    <Link
      to={"/logs/" + encodeURIComponent(log.requestId)}
      className="focus-ring grid gap-3 px-5 py-4 transition-colors hover:bg-surface-hover md:grid-cols-[minmax(0,1fr)_auto_auto] md:items-center"
    >
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <span
            className={
              "rounded-full px-2.5 py-1 text-xs font-semibold " + statusTone(log.statusCode)
            }
          >
            {log.statusCode}
          </span>
          <span className="text-sm font-semibold">{log.publicModel ?? "未解析模型"}</span>
          {log.isStream ? (
            <span className="rounded-full bg-surface-muted px-2.5 py-1 text-xs text-secondary">
              流式
            </span>
          ) : null}
        </div>
        <p className="mt-2 truncate font-mono text-xs text-tertiary" title={log.requestId}>
          {log.requestId}
        </p>
      </div>
      <div className="text-left text-xs text-secondary md:text-right">
        <p>{protocolLabel(log.ingressProtocol)}</p>
        <p className="mt-1">
          {log.attemptCount} 次 Attempt · {log.latencyMs} ms
        </p>
      </div>
      <time
        className="text-xs tabular-nums text-tertiary md:text-right"
        dateTime={new Date(log.startedAtMs).toISOString()}
      >
        {formatDate(log.startedAtMs)}
      </time>
    </Link>
  );
}

function protocolLabel(value: RequestLog["ingressProtocol"]) {
  return value === "anthropic_messages" ? "Claude Messages" : "Codex Responses";
}

function statusTone(status: number) {
  if (status >= 200 && status < 300) {
    return "bg-success/15 text-success-copy";
  }
  if (status >= 400 && status < 500) {
    return "bg-warning/15 text-warning-copy";
  }
  return "bg-danger/15 text-danger-copy";
}

function formatDate(milliseconds: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(milliseconds);
}
