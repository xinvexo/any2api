import type {
  RequestLog,
  RequestLogOperation,
  RequestLogOutcome,
  RequestLogProtocol,
} from "../api/request-log-contracts";

export type UpstreamSourceKind = "api_key" | "oauth" | "none";

export function protocolLabel(value: RequestLogProtocol) {
  switch (value) {
    case "openai_chat_completions":
      return "Chat Completions";
    case "openai_images":
      return "Images";
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
    case "alpha_search":
      return "/v1/alpha/search";
    case "chat_completions":
      return "/v1/chat/completions";
    case "images_generations":
      return "/v1/images/generations";
    case "images_edits":
      return "/v1/images/edits";
    case "messages":
      return "/v1/messages";
    case "messages_count_tokens":
      return "/v1/messages/count_tokens";
  }
}

type UpstreamSourceFields = {
  oauthAccountId: string | null;
  credentialId: string | null;
  providerEndpointId?: string | null;
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

/** User-facing source label that keeps API-key credentials identifiable. */
export function upstreamCredentialDisplay(log: UpstreamSourceFields) {
  const source = upstreamSource(log);
  if (source.kind === "oauth") {
    return { label: "上游凭据", value: `OAuth · ${source.displayName}` };
  }
  if (source.kind === "api_key") {
    const endpoint =
      log.providerEndpointName?.trim() ||
      (log.providerEndpointId ? shortId(log.providerEndpointId) : null);
    const credential = log.credentialLabel?.trim() || source.shortId;
    return {
      label: "上游凭据",
      value: endpoint ? `${endpoint} · ${credential}` : credential,
    };
  }
  return { label: "上游凭据", value: "未记录" };
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

export function isSuccessOutcome(outcome: RequestLogOutcome) {
  return outcome === "success";
}

/** Hide the timeline only for a direct, single-attempt success. */
export function shouldShowAttemptTimeline(
  outcome: RequestLogOutcome,
  attemptCount: number,
) {
  return !isSuccessOutcome(outcome) || attemptCount > 1;
}

/** Final stream result is independent from the initially committed HTTP status. */
export function resultBadgeLabel(outcome: RequestLogOutcome, status: number) {
  if (outcome === "success") {
    return "成功";
  }
  if (outcome === "cancelled") {
    return "已取消";
  }
  return `失败 ${status}`;
}

export function resultTone(outcome: RequestLogOutcome, status: number) {
  if (outcome === "success") {
    return "bg-success/10 text-success";
  }
  if (outcome === "cancelled") {
    return "bg-warning/12 text-warning";
  }
  if (status >= 400 && status < 500) {
    return "bg-warning/12 text-warning";
  }
  return "bg-danger/10 text-danger";
}

export function processingTone() {
  return "bg-accent/10 text-accent-copy";
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
