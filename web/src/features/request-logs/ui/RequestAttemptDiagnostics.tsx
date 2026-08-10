import type {
  RequestAttempt,
  RequestAttemptTransport,
  RequestTransportResolverMode,
  RequestTransportTrafficClass,
} from "../api/request-log-contracts";

export function RequestAttemptDiagnostics({
  attempt,
  compact = false,
}: {
  attempt: RequestAttempt;
  compact?: boolean;
}) {
  if (attempt.transport === null && attempt.streamTiming === null) {
    return null;
  }
  return (
    <div
      className={
        compact
          ? "mt-1.5 border-t border-subtle/70 pt-1.5 text-[10px] text-tertiary"
          : "mt-2 rounded-[10px] border border-subtle bg-surface-muted/45 px-3 py-2 text-[11px] text-secondary"
      }
    >
      {attempt.transport ? <TransportSummary transport={attempt.transport} /> : null}
      {attempt.streamTiming ? (
        <dl className="mt-1 grid grid-cols-2 gap-x-3 gap-y-1 sm:grid-cols-4">
          <Timing label="上游首帧" value={attempt.streamTiming.firstUpstreamFrameMs} />
          <Timing label="预提交 Commit" value={attempt.streamTiming.streamCommitMs} />
          <Timing label="下游首字节" value={attempt.streamTiming.firstDownstreamByteMs} />
          <Timing label="取消" value={attempt.streamTiming.streamCancelMs} />
        </dl>
      ) : null}
    </div>
  );
}

function TransportSummary({ transport }: { transport: RequestAttemptTransport }) {
  return (
    <p className="break-words [overflow-wrap:anywhere]">
      <span className="font-medium text-primary">
        {transport.wireProfileId} · wire v{transport.wireProfileVersion}
      </span>
      {" · timeout v" + transport.timeoutPolicyVersion}
      {" · " + resolverLabel(transport.resolverMode)}
      {" · " + transport.proxyKind.toUpperCase()}
      {` · 连接/读取/池 ${transport.connectTimeoutMs}/${transport.readTimeoutMs}/${transport.poolIdleTimeoutMs} ms`}
      {` · 隔离代际 ${transport.routingGeneration}/${transport.authenticationVersion}`}
      {" · " + trafficClassLabel(transport.trafficClass)}
    </p>
  );
}

function Timing({ label, value }: { label: string; value: number | null }) {
  return (
    <div>
      <dt className="text-tertiary">{label}</dt>
      <dd className="font-medium text-primary">{value === null ? "未记录" : `${value} ms`}</dd>
    </div>
  );
}

function resolverLabel(value: RequestTransportResolverMode) {
  switch (value) {
    case "system":
      return "系统 DNS";
    case "proxy_remote":
      return "代理远端 DNS";
    case "local_cached":
      return "本地缓存 DNS";
  }
}

function trafficClassLabel(value: RequestTransportTrafficClass) {
  switch (value) {
    case "data_plane":
      return "数据面";
    case "oauth_token":
      return "OAuth Token";
    case "oauth_quota":
      return "OAuth 额度";
    case "diagnostic":
      return "诊断流量";
  }
}
