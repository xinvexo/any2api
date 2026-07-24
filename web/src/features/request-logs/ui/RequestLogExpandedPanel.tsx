import type { RequestAttempt, RequestLog } from "../api/request-log-contracts";
import {
  formatDurationMs,
  formatMetric,
  formatTokenCount,
  operationLabel,
  protocolLabel,
  resultLabel,
  shortId,
  statusTone,
  totalTokens,
  upstreamKindTone,
  upstreamSource,
} from "../model/request-log-presentation";
import {
  getRequestLogErrorMessage,
  isRequestLogNotFound,
} from "../model/request-log-error";
import { useRequestLog } from "../model/use-request-logs";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/Button";

export function RequestLogExpandedPanel({
  requestId,
  summary,
}: {
  requestId: string;
  summary: RequestLog;
}) {
  const query = useRequestLog(requestId);

  if (query.isPending && !query.data) {
    return <p className="text-[12px] text-secondary">正在读取详情…</p>;
  }

  if (!query.data) {
    if (isRequestLogNotFound(query.error)) {
      return (
        <p className="text-[12px] text-secondary">
          这条请求日志不存在，可能已超过保留期限。
        </p>
      );
    }
    return (
      <div className="space-y-2">
        <p className="text-[12px] text-danger">{getRequestLogErrorMessage(query.error)}</p>
        <Button size="sm" variant="ghost" onClick={() => void query.refetch()}>
          重试
        </Button>
      </div>
    );
  }

  const { request, attempts } = query.data;
  const source = upstreamSource(request);
  const model = request.publicModel ?? summary.publicModel;
  const tokens = totalTokens(request);

  return (
    <div className="space-y-3">
      <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-secondary">
        {source.kind !== "none" ? (
          <>
            <span
              className={cn(
                "rounded-full px-1.5 py-0.5 text-[10px] font-medium",
                upstreamKindTone(source.kind),
              )}
            >
              {source.kindLabel}
            </span>
            <span className="font-mono font-medium text-primary">{source.shortId}</span>
            <span className="text-tertiary">·</span>
          </>
        ) : (
          <span className="text-tertiary">未选上游 ·</span>
        )}
        <span className="min-w-0 truncate font-medium text-primary">{model ?? "未解析模型"}</span>
        <span className="text-tertiary">·</span>
        <span>{protocolLabel(request.ingressProtocol)}</span>
        <span className="text-tertiary">·</span>
        <span>{operationLabel(request.operation)}</span>
        <span className="text-tertiary">·</span>
        <span>{request.isStream ? "流式" : "JSON"}</span>
        <span className="text-tertiary">·</span>
        <span
          className={cn(
            "font-medium",
            statusTone(request.statusCode).includes("success") ? "text-success" : "text-danger",
          )}
        >
          {resultLabel(request.statusCode)} ({request.statusCode})
        </span>
      </div>

      {request.errorMessage || request.errorClass ? (
        <div className="rounded-[10px] border border-danger/20 bg-danger/5 px-2.5 py-2">
          <p className="text-[11px] font-medium text-danger">错误详情</p>
          <p className="mt-1 break-all text-[12px] font-medium text-primary [overflow-wrap:anywhere]">
            {request.errorMessage ?? "未记录具体消息"}
          </p>
          {request.errorClass ? (
            <p className="mt-1 text-[11px] text-tertiary">分类 · {request.errorClass}</p>
          ) : null}
        </div>
      ) : null}

      <dl className="grid grid-cols-2 gap-x-3 gap-y-2 text-[11px] sm:grid-cols-3 lg:grid-cols-4">
        <Detail label="请求 ID" value={request.requestId} />
        <Detail label="状态" value={`HTTP ${request.statusCode}`} />
        <Detail label="错误分类" value={request.errorClass ?? "无"} />
        <Detail label="错误消息" value={request.errorMessage ?? "无"} />
        <Detail label="首字延迟" value={formatDurationMs(request.firstTokenMs)} />
        <Detail label="总延迟" value={formatDurationMs(request.latencyMs)} />
        <Detail label="总 Token" value={formatTokenCount(tokens)} />
        <Detail label="输入 Token" value={formatMetric(request.inputTokens)} />
        <Detail label="输出 Token" value={formatMetric(request.outputTokens)} />
        <Detail
          label="缓存读/写"
          value={`${formatMetric(request.cacheReadTokens)} / ${formatMetric(request.cacheWriteTokens)}`}
        />
        <Detail label="Attempt" value={String(request.attemptCount)} />
        <Detail label="出口代理" value={shortId(request.proxyProfileId)} />
        <Detail label="网关密钥" value={shortId(request.gatewayApiKeyId)} />
      </dl>

      <div>
        <p className="text-[11px] font-medium text-secondary">Attempt 时间线</p>
        {attempts.length === 0 ? (
          <p className="mt-1.5 text-[11px] text-tertiary">没有可展示的 Attempt</p>
        ) : (
          <ul className="mt-1.5 space-y-1.5">
            {attempts.map((attempt) => (
              <AttemptLine key={attempt.attemptNo} attempt={attempt} />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function AttemptLine({ attempt }: { attempt: RequestAttempt }) {
  const source = upstreamSource(attempt);
  return (
    <li className="min-w-0 space-y-1 rounded-[10px] bg-surface/80 px-2.5 py-1.5 text-[11px]">
      <div className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-0.5">
        <span className="shrink-0 font-semibold tabular-nums text-primary">#{attempt.attemptNo}</span>
        <span className="font-medium text-primary">{attempt.outcome}</span>
        <span className="text-secondary">{attempt.statusCode ?? "未收到状态"}</span>
        <span className="tabular-nums text-tertiary">{formatDurationMs(attempt.durationMs)}</span>
        {source.kind !== "none" ? (
          <>
            <span
              className={cn(
                "rounded-full px-1.5 py-px text-[10px] font-medium",
                upstreamKindTone(source.kind),
              )}
            >
              {source.kindLabel}
            </span>
            <span className="min-w-0 truncate font-mono text-tertiary">
              {source.shortId} · Proxy {shortId(attempt.proxyProfileId)}
            </span>
          </>
        ) : (
          <span className="text-tertiary">未选上游 · Proxy {shortId(attempt.proxyProfileId)}</span>
        )}
        {attempt.errorClass ? (
          <span className="text-tertiary">{attempt.errorClass}</span>
        ) : null}
      </div>
      {attempt.errorMessage ? (
        <p className="break-all text-[11px] text-danger [overflow-wrap:anywhere]">
          {attempt.errorMessage}
        </p>
      ) : null}
    </li>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
      <dt className="text-tertiary">{label}</dt>
      <dd className="mt-0.5 break-all font-medium text-primary [overflow-wrap:anywhere]">
        {value}
      </dd>
    </div>
  );
}
