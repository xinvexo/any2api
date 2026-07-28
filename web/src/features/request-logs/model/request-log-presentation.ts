import type {
  RequestLog,
  RequestLogOperation,
  RequestLogProtocol,
} from "../api/request-log-contracts";

export type UpstreamSourceKind = "api_key" | "oauth" | "none";

export function protocolLabel(value: RequestLogProtocol) {
  switch (value) {
    case "openai_chat_completions":
      return "Chat Completions";
    case "anthropic_messages":
      return "Messages";
    case "openai_responses":
      return "Responses";
  }
}

/** Public gateway path for the logged operation (matches `/v1/...` routes). */
export function operationLabel(value: RequestLogOperation) {
  switch (value) {
    case "responses":
      return "/v1/responses";
    case "responses_compact":
      return "/v1/responses/compact";
    case "chat_completions":
      return "/v1/chat/completions";
    case "messages":
      return "/v1/messages";
    case "messages_count_tokens":
      return "/v1/messages/count_tokens";
  }
}

type UpstreamSourceFields = {
  oauthAccountId: string | null;
  credentialId: string | null;
  providerEndpointName?: string | null;
  oauthAccountLabel?: string | null;
  credentialLabel?: string | null;
};

/** Final upstream routing source — Provider API Key or OAuth account, never Gateway Key. */
export function upstreamSource(log: UpstreamSourceFields): {
  kind: UpstreamSourceKind;
  kindLabel: string;
  id: string | null;
  /** Prefer human label; fall back to short id when the account was deleted. */
  displayName: string;
  shortId: string;
} {
  if (log.oauthAccountId) {
    const short = shortId(log.oauthAccountId);
    return {
      kind: "oauth",
      kindLabel: "OAuth",
      id: log.oauthAccountId,
      displayName: log.oauthAccountLabel?.trim() || short,
      shortId: short,
    };
  }
  if (log.credentialId) {
    const short = shortId(log.credentialId);
    const credentialName = log.credentialLabel?.trim() || short;
    const endpointName = log.providerEndpointName?.trim();
    return {
      kind: "api_key",
      kindLabel: "API Key",
      id: log.credentialId,
      displayName: endpointName ? `${endpointName}-${credentialName}` : credentialName,
      shortId: short,
    };
  }
  return {
    kind: "none",
    kindLabel: "未选择",
    id: null,
    displayName: "—",
    shortId: "—",
  };
}

function shortId(value: string | null | undefined) {
  return value ? `${value.slice(0, 8)}…` : "未记录";
}

/** Prefer proxy profile name from the live snapshot; fall back to short id. */
export function proxyDisplayName(
  id: string | null | undefined,
  label?: string | null,
) {
  const trimmed = label?.trim();
  if (trimmed) {
    return trimmed;
  }
  return shortId(id);
}

export function isSuccessStatus(status: number) {
  return status >= 200 && status < 300;
}

/** List badge text: success plain, failure includes HTTP status. */
export function resultBadgeLabel(status: number) {
  return isSuccessStatus(status) ? "成功" : `失败 ${status}`;
}

export function statusTone(status: number) {
  if (isSuccessStatus(status)) {
    return "bg-success/10 text-success";
  }
  if (status >= 400 && status < 500) {
    return "bg-warning/12 text-warning";
  }
  return "bg-danger/10 text-danger";
}

export function upstreamKindTone(kind: UpstreamSourceKind) {
  if (kind === "oauth") {
    return "bg-accent/10 text-accent-copy";
  }
  if (kind === "api_key") {
    return "bg-surface-muted text-secondary";
  }
  return "bg-surface-muted text-tertiary";
}

export function formatLogTime(milliseconds: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(milliseconds);
}

/** Prefer seconds for human scan, like the reference log table. */
export function formatDurationMs(value: number | null) {
  if (value === null) {
    return "—";
  }
  if (value < 1000) {
    return `${value} ms`;
  }
  return `${(value / 1000).toFixed(2)} s`;
}

export function formatTokenCount(value: number | null) {
  return value === null ? "—" : value.toLocaleString();
}

/** Compact list latency: first-token / total. */
export function formatLatencyPair(firstTokenMs: number | null, latencyMs: number) {
  return `${formatDurationMs(firstTokenMs)} / ${formatDurationMs(latencyMs)}`;
}

/**
 * Output TPS from generation window.
 * Prefer post-first-token duration for streams; otherwise full request latency.
 */
export function outputTps(
  log: Pick<RequestLog, "outputTokens" | "latencyMs" | "firstTokenMs">,
): number | null {
  if (log.outputTokens === null || log.outputTokens <= 0 || log.latencyMs <= 0) {
    return null;
  }
  const generationMs =
    log.firstTokenMs !== null && log.latencyMs > log.firstTokenMs
      ? log.latencyMs - log.firstTokenMs
      : log.latencyMs;
  if (generationMs <= 0) {
    return null;
  }
  return log.outputTokens / (generationMs / 1_000);
}

export function formatTps(value: number | null) {
  if (value === null) {
    return "—";
  }
  if (value >= 100) {
    return `${Math.round(value)} t/s`;
  }
  if (value >= 10) {
    return `${value.toFixed(1)} t/s`;
  }
  return `${value.toFixed(2)} t/s`;
}

/**
 * Compact list tokens:
 * 入/出 · 命中/创建 · TPS
 */
export function formatTokenSummary(
  log: Pick<
    RequestLog,
    | "inputTokens"
    | "outputTokens"
    | "cacheReadTokens"
    | "cacheWriteTokens"
    | "latencyMs"
    | "firstTokenMs"
  >,
) {
  if (
    log.inputTokens === null
    && log.outputTokens === null
    && log.cacheReadTokens === null
    && log.cacheWriteTokens === null
  ) {
    return "—";
  }
  const io = `${formatTokenCount(log.inputTokens)}/${formatTokenCount(log.outputTokens)}`;
  const cache = `${formatTokenCount(log.cacheReadTokens)}/${formatTokenCount(log.cacheWriteTokens)}`;
  return `${io} · ${cache} · ${formatTps(outputTps(log))}`;
}
