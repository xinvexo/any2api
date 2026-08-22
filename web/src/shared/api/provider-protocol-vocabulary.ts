export const PROVIDER_KINDS = ["openai", "codex", "claude", "grok", "kimi"] as const;
export type ProviderKind = (typeof PROVIDER_KINDS)[number];

export const PROTOCOL_DIALECTS = [
  "openai_responses",
  "openai_chat_completions",
  "openai_images",
  "anthropic_messages",
] as const;
export type ProtocolDialect = (typeof PROTOCOL_DIALECTS)[number];

export const PROTOCOL_OPERATIONS = [
  "responses",
  "responses_compact",
  "alpha_search",
  "chat_completions",
  "images_generations",
  "images_edits",
  "messages",
  "messages_count_tokens",
] as const;
export type ProtocolOperation = (typeof PROTOCOL_OPERATIONS)[number];

const PROVIDER_KIND_LABELS: Record<ProviderKind, string> = {
  openai: "OpenAI",
  codex: "Codex",
  claude: "Claude",
  grok: "Grok",
  kimi: "Kimi",
};

const PROTOCOL_DIALECT_LABELS: Record<ProtocolDialect, string> = {
  openai_responses: "OpenAI Responses",
  openai_chat_completions: "OpenAI Chat Completions",
  openai_images: "OpenAI Images",
  anthropic_messages: "Anthropic Messages",
};

const PROTOCOL_OPERATION_LABELS: Record<ProtocolOperation, string> = {
  responses: "响应生成",
  responses_compact: "响应压缩",
  alpha_search: "联网搜索",
  chat_completions: "聊天补全",
  images_generations: "图像生成",
  images_edits: "图像编辑",
  messages: "消息",
  messages_count_tokens: "Token 计数",
};

export function isProviderKind(value: unknown): value is ProviderKind {
  return typeof value === "string" && PROVIDER_KINDS.includes(value as ProviderKind);
}

export function isProtocolDialect(value: unknown): value is ProtocolDialect {
  return typeof value === "string"
    && PROTOCOL_DIALECTS.includes(value as ProtocolDialect);
}

export function isProtocolOperation(value: unknown): value is ProtocolOperation {
  return typeof value === "string"
    && PROTOCOL_OPERATIONS.includes(value as ProtocolOperation);
}

export function providerKindLabel(kind: ProviderKind) {
  return PROVIDER_KIND_LABELS[kind];
}

export function protocolDialectLabel(dialect: ProtocolDialect) {
  return PROTOCOL_DIALECT_LABELS[dialect];
}

export function protocolOperationLabel(operation: ProtocolOperation) {
  return PROTOCOL_OPERATION_LABELS[operation];
}

export function protocolDialectForOperation(
  operation: ProtocolOperation,
): ProtocolDialect {
  if (
    operation === "responses"
    || operation === "responses_compact"
    || operation === "alpha_search"
  ) {
    return "openai_responses";
  }
  if (operation === "chat_completions") {
    return "openai_chat_completions";
  }
  if (operation === "images_generations" || operation === "images_edits") {
    return "openai_images";
  }
  return "anthropic_messages";
}
