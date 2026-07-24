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
    case "codex_backend":
      return "Codex Backend";
    case "anthropic_messages":
      return "Messages";
    case "openai_responses":
      return "Responses";
  }
}

export function operationLabel(value: RequestLogOperation) {
  switch (value) {
    case "responses":
      return "responses";
    case "responses_compact":
      return "compact";
    case "chat_completions":
      return "chat";
    case "messages":
      return "messages";
    case "messages_count_tokens":
      return "count_tokens";
  }
}

/** Final upstream routing source — Provider API Key or OAuth account, never Gateway Key. */
export function upstreamSource(log: Pick<RequestLog, "oauthAccountId" | "credentialId">): {
  kind: UpstreamSourceKind;
  kindLabel: string;
  id: string | null;
  shortId: string;
} {
  if (log.oauthAccountId) {
    return {
      kind: "oauth",
      kindLabel: "OAuth",
      id: log.oauthAccountId,
      shortId: shortId(log.oauthAccountId),
    };
  }
  if (log.credentialId) {
    return {
      kind: "api_key",
      kindLabel: "API Key",
      id: log.credentialId,
      shortId: shortId(log.credentialId),
    };
  }
  return {
    kind: "none",
    kindLabel: "未选择",
    id: null,
    shortId: "—",
  };
}

export function upstreamLabel(log: Pick<RequestLog, "oauthAccountId" | "credentialId">) {
  const source = upstreamSource(log);
  if (source.kind === "none") {
    return "未选择上游";
  }
  return `${source.kindLabel} ${source.shortId}`;
}

export function shortId(value: string | null | undefined) {
  return value ? `${value.slice(0, 8)}…` : "未记录";
}

export function isSuccessStatus(status: number) {
  return status >= 200 && status < 300;
}

export function resultLabel(status: number) {
  return isSuccessStatus(status) ? "成功" : "失败";
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

export function totalTokens(log: Pick<RequestLog, "inputTokens" | "outputTokens">) {
  if (log.inputTokens === null && log.outputTokens === null) {
    return null;
  }
  return (log.inputTokens ?? 0) + (log.outputTokens ?? 0);
}

export function formatMetric(value: number | null, suffix = "") {
  return value === null ? "未记录" : value.toLocaleString() + suffix;
}

export function formatTokenCount(value: number | null) {
  return value === null ? "—" : value.toLocaleString();
}
