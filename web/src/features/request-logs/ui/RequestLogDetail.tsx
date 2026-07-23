import { ArrowLeft, RefreshCw } from "lucide-react";
import { Link } from "react-router-dom";

import type { RequestAttempt } from "../api/request-log-contracts";
import { getRequestLogErrorMessage, isRequestLogNotFound } from "../model/request-log-error";
import { useRequestLog } from "../model/use-request-logs";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export function RequestLogDetail({ requestId }: { requestId: string }) {
  const query = useRequestLog(requestId);

  if (query.isPending && !query.data) {
    return (
      <Surface
        className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary"
        aria-busy="true"
      >
        正在读取请求详情
      </Surface>
    );
  }
  if (!query.data) {
    if (isRequestLogNotFound(query.error)) {
      return (
        <Surface className="p-6" role="status">
          <p className="font-semibold">这条请求日志不存在</p>
          <p className="mt-2 text-sm text-secondary">
            记录可能已经超过保留期限，或因容量上限被清理。
          </p>
          <Link
            to="/logs"
            className="focus-ring mt-5 inline-flex h-10 items-center gap-2 rounded-control border border-subtle bg-surface px-4 text-sm font-semibold text-primary hover:bg-surface-hover"
          >
            <ArrowLeft size={15} />
            返回请求日志
          </Link>
        </Surface>
      );
    }
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取这条请求</p>
        <p className="mt-2 text-sm text-secondary">{getRequestLogErrorMessage(query.error)}</p>
        <div className="mt-5 flex flex-wrap gap-3">
          <Button onClick={() => void query.refetch()} disabled={query.isFetching}>
            <RefreshCw size={15} />
            重试
          </Button>
          <Link
            to="/logs"
            className="focus-ring inline-flex h-10 items-center gap-2 rounded-control px-4 text-sm font-semibold text-secondary hover:bg-surface-hover hover:text-primary"
          >
            <ArrowLeft size={15} />
            返回请求日志
          </Link>
        </div>
      </Surface>
    );
  }

  const { request, attempts } = query.data;
  return (
    <div className="space-y-5" aria-busy={query.isFetching}>
      <Link
        to="/logs"
        className="focus-ring inline-flex items-center gap-2 text-sm font-semibold text-secondary hover:text-primary"
      >
        <ArrowLeft size={15} />
        返回请求日志
      </Link>

      <Surface className="p-5 sm:p-6">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="min-w-0">
            <p className="text-sm text-secondary">{request.isStream ? "流式请求" : "JSON 请求"}</p>
            <h2 className="mt-1 truncate text-xl font-semibold">
              {request.publicModel ?? "未解析模型"}
            </h2>
            <code className="mt-2 block break-all text-xs text-tertiary">{request.requestId}</code>
          </div>
          <span
            className={
              "self-start rounded-full px-3 py-1 text-sm font-semibold " +
              statusTone(request.statusCode)
            }
          >
            {request.statusCode}
          </span>
        </div>

        <dl className="mt-6 grid gap-4 text-sm sm:grid-cols-2 lg:grid-cols-4">
          <Detail
            label="协议"
            value={
              request.ingressProtocol === "anthropic_messages"
                ? "Claude Messages"
                : "Codex Responses"
            }
          />
          <Detail label="操作" value={request.operation} />
          <Detail label="延迟" value={request.latencyMs + " ms"} />
          <Detail label="Attempt" value={String(request.attemptCount)} />
          <Detail label="错误分类" value={request.errorClass ?? "无"} />
          <Detail label="Credential" value={shortId(request.credentialId)} />
          <Detail label="出口代理" value={shortId(request.proxyProfileId)} />
        </dl>

        <div className="mt-6 border-t border-subtle pt-5">
          <h3 className="font-semibold">Token 遥测</h3>
          <p className="mt-1 text-sm text-secondary">
            首 Token 延迟由本机在首个内容帧交付时测量；Token 计数仅取上游协议明确返回的字段，非流式请求不估算延迟。
          </p>
          <dl className="mt-4 grid gap-4 text-sm sm:grid-cols-2 lg:grid-cols-5">
            <Detail
              label="首 Token 延迟（TTFT）"
              value={formatMetric(request.firstTokenMs, " ms")}
            />
            <Detail label="输入 Token" value={formatMetric(request.inputTokens)} />
            <Detail label="输出 Token" value={formatMetric(request.outputTokens)} />
            <Detail label="缓存读取" value={formatMetric(request.cacheReadTokens)} />
            <Detail label="缓存写入" value={formatMetric(request.cacheWriteTokens)} />
          </dl>
        </div>
      </Surface>

      <Surface className="overflow-hidden">
        <div className="border-b border-subtle px-5 py-4">
          <h2 className="font-semibold">Attempt 时间线</h2>
          <p className="mt-1 text-sm text-secondary">每次上游选择与最终结果按发生顺序记录。</p>
        </div>
        {attempts.length === 0 ? (
          <p className="px-5 py-8 text-center text-sm text-secondary">没有可展示的 Attempt</p>
        ) : (
          <div className="divide-y divide-subtle">
            {attempts.map((attempt) => (
              <AttemptRow key={attempt.attemptNo} attempt={attempt} />
            ))}
          </div>
        )}
      </Surface>
    </div>
  );
}

function AttemptRow({ attempt }: { attempt: RequestAttempt }) {
  return (
    <article className="grid gap-3 px-5 py-4 md:grid-cols-[auto_minmax(0,1fr)_auto] md:items-center">
      <span className="flex h-8 w-8 items-center justify-center rounded-full bg-surface-muted text-sm font-semibold tabular-nums">
        {attempt.attemptNo}
      </span>
      <div className="min-w-0">
        <p className="font-semibold">{attempt.outcome}</p>
        <p className="mt-1 break-all text-xs text-tertiary">
          Credential {shortId(attempt.credentialId)} · Proxy {shortId(attempt.proxyProfileId)}
        </p>
      </div>
      <div className="text-left text-xs text-secondary md:text-right">
        <p>{attempt.statusCode ?? "未收到状态"}</p>
        <p className="mt-1">{attempt.durationMs} ms</p>
      </div>
    </article>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs text-tertiary">{label}</dt>
      <dd className="mt-1 break-all font-medium">{value}</dd>
    </div>
  );
}

function shortId(value: string | null) {
  return value ? value.slice(0, 8) + "…" : "未记录";
}

function formatMetric(value: number | null, suffix = "") {
  return value === null ? "未记录" : value.toLocaleString() + suffix;
}

function statusTone(status: number) {
  if (status >= 200 && status < 300) {
    return "bg-success/15 text-success-copy";
  }
  if (status < 500) {
    return "bg-warning/15 text-warning-copy";
  }
  return "bg-danger/15 text-danger-copy";
}
