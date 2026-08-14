import type { OAuthRefreshFailure } from "../api/oauth-contracts";
import {
  formatOAuthRefreshFailureTime,
  oauthRefreshReasonLabel,
  oauthRefreshScopeLabel,
  oauthRefreshStageLabel,
  oauthRefreshTriggerLabel,
} from "../model/oauth-refresh-failure";

export function OAuthRefreshFailureNotice({
  failure,
}: {
  failure: OAuthRefreshFailure;
}) {
  const metadata = [
    failure.upstreamStatus === null ? null : `HTTP ${failure.upstreamStatus}`,
    oauthRefreshScopeLabel(failure.failureScope),
  ].filter((value): value is string => value !== null);

  return (
    <section
      className="min-w-0 py-0.5"
      aria-label="Token 刷新失败"
      role="alert"
    >
      <p className="text-[11px] font-semibold leading-4 text-danger">Token 刷新失败</p>
      <dl className="mt-1.5 grid grid-cols-[2rem_minmax(0,1fr)] gap-x-2 gap-y-1 text-[10px] leading-4">
        <dt className="text-tertiary">触发</dt>
        <dd className="min-w-0 text-secondary">{oauthRefreshTriggerLabel(failure.trigger)}</dd>
        <dt className="text-tertiary">阶段</dt>
        <dd className="min-w-0 text-secondary">{oauthRefreshStageLabel(failure.stage)}</dd>
        <dt className="text-tertiary">错误</dt>
        <dd className="min-w-0 text-secondary">
          {oauthRefreshReasonLabel(failure.reason)}
          {metadata.length > 0 ? `（${metadata.join(" · ")}）` : ""}
        </dd>
        <dt className="text-tertiary">时间</dt>
        <dd className="min-w-0 text-secondary tabular-nums">
          {formatOAuthRefreshFailureTime(failure.occurredAt)}
        </dd>
      </dl>
      <p className="mt-1.5 text-[10px] leading-4 text-danger">
        {failure.reauthorizationRequired
          ? "此认证材料已明确不可继续使用，请重新授权账号。"
          : "无需立即重新授权，请按失败阶段检查网络、代理或上游状态。"}
      </p>
    </section>
  );
}
