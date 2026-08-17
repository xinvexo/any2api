import { RefreshCw } from "lucide-react";

import type { RequestAttempt, RequestLogDetail } from "../api/request-log-contracts";
import { attemptDiagnosticSummary } from "../model/attempt-diagnostics";
import {
  getRequestLogErrorMessage,
  isRequestLogNotFound,
} from "../model/request-log-error";
import {
  formatDurationMs,
  formatLogTime,
  formatTps,
  isSuccessOutcome,
  operationLabel,
  outputTps,
  protocolLabel,
  proxyDisplayName,
  resultBadgeLabel,
  resultTone,
  shouldShowAttemptTimeline,
  upstreamSource,
} from "../model/request-log-presentation";
import { useRequestLog } from "../model/use-request-logs";
import { RequestAttemptDiagnostics } from "./RequestAttemptDiagnostics";
import { Button } from "@/shared/ui/Button";
import { SideDrawer } from "@/shared/ui/SideDrawer";

export function RequestLogDetailDrawer({
  requestId,
  onClose,
}: {
  requestId: string | null;
  onClose: () => void;
}) {
  const query = useRequestLog(requestId ?? "");
  const request = query.data?.request;
  return (
    <SideDrawer
      open={requestId !== null}
      title="请求详情"
      description={request ? `${request.publicModel ?? "未解析模型"} · ${request.requestId}` : "正在读取请求详情"}
      onClose={onClose}
      wide
    >
      {query.isPending && !query.data ? (
        <DetailSkeleton />
      ) : !query.data ? (
        <div className="space-y-3" role="alert">
          <p className="text-[13px] text-danger">
            {isRequestLogNotFound(query.error)
              ? "这条请求日志不存在，可能已超过保留期限。"
              : getRequestLogErrorMessage(query.error)}
          </p>
          <Button size="sm" variant="ghost" onClick={() => void query.refetch()}>
            <RefreshCw size={14} />
            重试
          </Button>
        </div>
      ) : (
        <RequestLogDrawerContent detail={query.data} />
      )}
    </SideDrawer>
  );
}

function RequestLogDrawerContent({
  detail,
}: {
  detail: RequestLogDetail;
}) {
  const { request, attempts, telemetry } = detail;
  const success = isSuccessOutcome(request.outcome);
  const source = upstreamSource(request);
  const showAttemptTimeline = shouldShowAttemptTimeline(
    request.outcome,
    request.attemptCount,
  );
  return (
    <div className="min-w-0 space-y-6">
      <div className="flex min-w-0 items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-[12px] text-secondary">{request.isStream ? "流式请求" : "JSON 请求"}</p>
          <h3 className="mt-1 truncate text-[17px] font-semibold">{request.publicModel ?? "未解析模型"}</h3>
          <code className="mt-2 block break-all text-[11px] text-tertiary">{request.requestId}</code>
        </div>
        <span className={`shrink-0 rounded-full px-2.5 py-1 text-[12px] font-medium ${resultTone(request.outcome, request.statusCode)}`}>
          {resultBadgeLabel(request.outcome, request.statusCode)}
        </span>
      </div>

      <dl className="grid gap-x-4 gap-y-3 text-[12px] sm:grid-cols-2">
        <Metric label="开始时间" value={formatLogTime(request.startedAtMs)} />
        <Metric label="协议" value={protocolLabel(request.ingressProtocol)} />
        <Metric label="接口" value={operationLabel(request.operation)} />
        <Metric label="客户端 IP" value={request.clientIp} mono />
        <Metric label="配置版本" value={String(request.configRevision)} mono />
        <Metric label="Gateway API Key" value={shortId(request.gatewayApiKeyId)} mono />
        <Metric label="思考级别" value={request.thinkingLevel ?? "未设置"} />
        <Metric label="HTTP 状态" value={String(request.statusCode)} />
        <Metric label="总耗时" value={formatDurationMs(request.latencyMs)} />
        <Metric label="首 Token 延迟" value={formatDurationMs(request.firstTokenMs)} />
        <Metric label="输入 Token" value={formatNullableMetric(request.inputTokens)} />
        <Metric label="输出 Token" value={formatNullableMetric(request.outputTokens)} />
        <Metric label="缓存命中 Token" value={formatNullableMetric(request.cacheReadTokens)} />
        <Metric label="缓存写入 Token" value={formatNullableMetric(request.cacheCreationTokens)} />
        <Metric label="TPS" value={success ? formatTps(outputTps(request)) : "未记录"} />
        <Metric label="Attempt" value={String(request.attemptCount)} />
        <Metric label="Provider Endpoint" value={request.providerEndpointName ?? shortId(request.providerEndpointId)} />
        <Metric label="上游凭据" value={`${source.kindLabel} · ${source.displayName}`} />
        <Metric label="出口代理" value={proxyDisplayName(request.proxyProfileId, request.proxyProfileLabel)} />
      </dl>

      {request.errorMessage ? (
        <section className="rounded-[8px] border border-danger/20 bg-danger/5 px-3 py-3">
          <h3 className="text-[12px] font-semibold text-danger">错误信息</h3>
          <p className="mt-1 break-all text-[12px] text-primary [overflow-wrap:anywhere]">{request.errorMessage}</p>
        </section>
      ) : null}

      <section className="border-t border-subtle pt-5">
        <h3 className="text-[14px] font-semibold">日志遥测</h3>
        <dl className="mt-3 grid grid-cols-2 gap-x-4 gap-y-3 text-[12px] sm:grid-cols-4">
          <Metric label="排队" value={telemetry.queuedRecords.toLocaleString()} />
          <Metric label="写入中" value={telemetry.inFlightRecords.toLocaleString()} />
          <Metric label="已丢弃" value={telemetry.droppedRecords.toLocaleString()} />
          <Metric label="已持久化" value={telemetry.persistedRecords.toLocaleString()} />
        </dl>
      </section>

      {showAttemptTimeline ? <section className="border-t border-subtle pt-5">
        <h3 className="text-[14px] font-semibold">Attempt 时间线</h3>
        {attempts.length === 0 ? (
          <p className="mt-2 text-[12px] text-secondary">没有可展示的 Attempt</p>
        ) : (
          <ol className="mt-3 space-y-2">
            {attempts.map((attempt) => <AttemptRow key={attempt.attemptNo} attempt={attempt} />)}
          </ol>
        )}
      </section> : null}
    </div>
  );
}

function AttemptRow({ attempt }: { attempt: RequestAttempt }) {
  const success = isSuccessOutcome(attempt.outcome);
  const diagnostic = attemptDiagnosticSummary(attempt);
  const source = upstreamSource(attempt);
  return (
    <li className="rounded-[8px] bg-surface-muted/55 px-3 py-2.5 text-[12px]">
      <div className="flex min-w-0 items-center justify-between gap-3">
        <span className={success ? "font-semibold" : "font-semibold text-danger"}>
          #{attempt.attemptNo} · {attempt.outcome === "cancelled" ? "已取消" : `HTTP ${attempt.statusCode ?? "-"}`}
        </span>
        <span className="shrink-0 tabular-nums text-secondary">
          {formatDurationMs(attempt.durationMs)}
        </span>
      </div>
      <p className="mt-1 break-all text-[11px] text-tertiary">
        {source.kindLabel} {source.displayName} · Route {shortId(attempt.routeTargetId)} · {proxyDisplayName(attempt.proxyProfileId, attempt.proxyProfileLabel)}
      </p>
      {attempt.errorMessage ? <p className="mt-1 break-all text-[11px] text-danger">{attempt.errorMessage}</p> : null}
      {diagnostic ? <p className="mt-1 break-words text-[11px] text-secondary">{diagnostic}</p> : null}
      <RequestAttemptDiagnostics attempt={attempt} compact />
    </li>
  );
}

function Metric({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="min-w-0">
      <dt className="text-tertiary">{label}</dt>
      <dd className={`mt-0.5 break-all text-primary ${mono ? "font-mono" : ""}`}>{value}</dd>
    </div>
  );
}

function DetailSkeleton() {
  return <div className="space-y-4" aria-busy="true"><div className="h-20 animate-pulse rounded-[8px] bg-surface-muted" /><div className="h-40 animate-pulse rounded-[8px] bg-surface-muted" /><div className="h-32 animate-pulse rounded-[8px] bg-surface-muted" /></div>;
}

function shortId(value: string | null) {
  return value ? `${value.slice(0, 8)}...` : "未记录";
}

function formatNullableMetric(value: number | null, suffix = "") {
  return value === null ? "未记录" : `${value.toLocaleString()}${suffix}`;
}
