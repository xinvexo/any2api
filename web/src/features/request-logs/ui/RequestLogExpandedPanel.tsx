import type {
  RequestAttempt,
  RequestLogOutcome,
} from "../api/request-log-contracts";
import { attemptDiagnosticSummary } from "../model/attempt-diagnostics";
import {
  formatDurationMs,
  isSuccessOutcome,
  operationLabel,
  protocolLabel,
  proxyDisplayName,
  shouldShowAttemptTimeline,
  upstreamKindTone,
  upstreamSource,
} from "../model/request-log-presentation";
import {
  getRequestLogErrorMessage,
  isRequestLogNotFound,
} from "../model/request-log-error";
import { useRequestLog } from "../model/use-request-logs";
import { RequestLogExpandedSkeleton } from "./RequestLogExpandedSkeleton";
import { RequestAttemptDiagnostics } from "./RequestAttemptDiagnostics";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/Button";
import { useAccordionReveal } from "@/shared/ui/use-accordion-reveal";

/**
 * Accordion detail: only fields the list row does not already show.
 * List already has time, token, model, thinking, result, latency, tokens/TPS.
 */
export function RequestLogExpandedPanel({
  requestId,
  outcome,
  attemptCount,
}: {
  requestId: string;
  outcome: RequestLogOutcome;
  attemptCount: number;
}) {
  const query = useRequestLog(requestId);
  const initialPending = query.isPending && !query.data;
  const revealContent = useAccordionReveal(true, !initialPending);
  const expectedFailure = !isSuccessOutcome(outcome);
  const expectedTimeline = shouldShowAttemptTimeline(outcome, attemptCount);

  if (initialPending || !revealContent) {
    return (
      <RequestLogExpandedSkeleton
        failed={expectedFailure}
        showAttemptTimeline={expectedTimeline}
      />
    );
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
  const success = isSuccessOutcome(request.outcome);
  const showAttemptTimeline = shouldShowAttemptTimeline(
    request.outcome,
    request.attemptCount,
  );

  return (
    <div className="min-w-0 max-w-full space-y-3 overflow-x-clip">
      <dl className="grid min-w-0 grid-cols-2 gap-x-3 gap-y-2 text-[11px] sm:grid-cols-3 lg:grid-cols-4">
        <Detail label="请求 ID" value={request.requestId} />
        <Detail label="协议" value={protocolLabel(request.ingressProtocol)} />
        {/* Gateway operation endpoint (responses / compact / count_tokens…), not a user action. */}
        <Detail label="接口" value={operationLabel(request.operation)} />
        <Detail label="形态" value={request.isStream ? "流式" : "JSON"} />
        <Detail label="HTTP" value={String(request.statusCode)} />
        <Detail
          label="出口代理"
          value={proxyDisplayName(request.proxyProfileId, request.proxyProfileLabel)}
        />
        {!success && request.errorMessage ? (
          <Detail label="错误消息" value={request.errorMessage} />
        ) : null}
      </dl>

      {showAttemptTimeline ? (
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
      ) : null}
    </div>
  );
}

function AttemptLine({ attempt }: { attempt: RequestAttempt }) {
  const source = upstreamSource(attempt);
  const failed = !isSuccessOutcome(attempt.outcome);
  const diagnostic = attemptDiagnosticSummary(attempt);
  const upstreamIdentity = source.displayName;
  const proxyIdentity = proxyDisplayName(
    attempt.proxyProfileId,
    attempt.proxyProfileLabel,
  );
  return (
    <li className="min-w-0 rounded-[10px] bg-surface/80 px-2.5 py-1.5 text-[11px]">
      {/* Single flow: stay on one line when it fits; wrap only when the row runs out of width. */}
      <div className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-0.5">
        <span className="shrink-0 font-semibold tabular-nums text-primary">#{attempt.attemptNo}</span>
        <span className={cn("shrink-0 font-medium", failed ? "text-danger" : "text-primary")}>
          {attempt.outcome === "cancelled"
            ? "已取消"
            : attempt.statusCode === null
              ? "未收到上游状态"
              : failed
                ? `失败 · HTTP ${attempt.statusCode}`
                : `HTTP ${attempt.statusCode}`}
        </span>
        <span className="shrink-0 tabular-nums text-tertiary">
          {formatDurationMs(attempt.durationMs)}
        </span>
        {source.kind !== "none" ? (
          <>
            <span
              className={cn(
                "inline-flex max-w-full shrink-0 truncate rounded-full px-1.5 py-px text-[11px] font-medium",
                upstreamKindTone(source.kind),
              )}
              title={[source.kindLabel, upstreamIdentity, source.id ? `(${source.id})` : null]
                .filter(Boolean)
                .join(" ")}
            >
              {upstreamIdentity}
            </span>
            <span className="shrink-0 text-tertiary">· {proxyIdentity}</span>
          </>
        ) : (
          <span className="shrink-0 text-tertiary">未选上游 · {proxyIdentity}</span>
        )}
        {attempt.errorMessage ? (
          <span className="min-w-0 break-words text-danger [overflow-wrap:anywhere]">
            {attempt.errorMessage}
          </span>
        ) : null}
        {diagnostic ? (
          <span className="min-w-0 break-words text-secondary">{diagnostic}</span>
        ) : null}
      </div>
      <RequestAttemptDiagnostics attempt={attempt} compact />
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
