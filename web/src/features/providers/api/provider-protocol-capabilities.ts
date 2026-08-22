export type ProviderKind = "openai" | "codex" | "claude" | "grok" | "kimi";
export type ProtocolDialect =
  | "openai_responses"
  | "openai_chat_completions"
  | "openai_images"
  | "anthropic_messages";
export type ProtocolOperation =
  | "responses"
  | "responses_compact"
  | "alpha_search"
  | "chat_completions"
  | "images_generations"
  | "images_edits"
  | "messages"
  | "messages_count_tokens";
export type ProtocolFidelity = "direct" | "translated";
export type BridgeRequestFieldBehavior =
  | "forwarded"
  | "translated"
  | "validated_only"
  | "local_state";

export interface BridgeRequestFieldCapability {
  path: string;
  behavior: BridgeRequestFieldBehavior;
}

export interface BridgeLimitation {
  code: string;
  description: string;
}

export interface ProtocolBridgeCapability {
  contractId: string;
  requestFields: BridgeRequestFieldCapability[];
  toolTypes: string[];
  limitations: BridgeLimitation[];
}

export interface ProviderUpstreamProtocolOption {
  protocol: ProtocolDialect;
  fidelity: ProtocolFidelity;
  operations: ProtocolOperation[];
  bridge: ProtocolBridgeCapability | null;
}

export interface ProviderProtocolOptions {
  providerKind: ProviderKind;
  acceptedProtocol: ProtocolDialect;
  upstreamOptions: ProviderUpstreamProtocolOption[];
}

export function parseProviderProtocolOptions(value: unknown): ProviderProtocolOptions {
  if (!isRecord(value) || !Array.isArray(value.upstream_options)) {
    throw invalid();
  }
  const acceptedProtocol = readProtocolDialect(value.accepted_protocol);
  const upstreamOptions = value.upstream_options.map((option) =>
    parseUpstreamOption(option, acceptedProtocol),
  );
  if (new Set(upstreamOptions.map((option) => option.protocol)).size !== upstreamOptions.length) {
    throw invalid();
  }
  return {
    providerKind: readProviderKind(value.provider_kind),
    acceptedProtocol,
    upstreamOptions,
  };
}

function parseUpstreamOption(
  value: unknown,
  acceptedProtocol: ProtocolDialect,
): ProviderUpstreamProtocolOption {
  if (!isRecord(value) || !Array.isArray(value.operations)) throw invalid();
  const protocol = readProtocolDialect(value.protocol);
  const fidelity = readProtocolFidelity(value.fidelity);
  const operations = value.operations.map(readProtocolOperation);
  const bridge = value.bridge === null ? null : parseBridgeCapability(value.bridge);
  if (
    operations.length === 0 ||
    new Set(operations).size !== operations.length ||
    operations.some((operation) => operationDialect(operation) !== acceptedProtocol) ||
    (fidelity === "direct" && (protocol !== acceptedProtocol || bridge !== null)) ||
    (fidelity === "translated" && (protocol === acceptedProtocol || bridge === null))
  ) {
    throw invalid();
  }
  return { protocol, fidelity, operations, bridge };
}

function parseBridgeCapability(value: unknown): ProtocolBridgeCapability {
  if (
    !isRecord(value) ||
    !Array.isArray(value.request_fields) ||
    !Array.isArray(value.tool_types) ||
    !Array.isArray(value.limitations)
  ) {
    throw invalid();
  }
  const requestFields = value.request_fields.map((field) => {
    if (!isRecord(field)) throw invalid();
    return { path: readString(field.path), behavior: readFieldBehavior(field.behavior) };
  });
  const toolTypes = value.tool_types.map(readString);
  const limitations = value.limitations.map((limitation) => {
    if (!isRecord(limitation)) throw invalid();
    return {
      code: readString(limitation.code),
      description: readString(limitation.description),
    };
  });
  if (
    new Set(requestFields.map((field) => field.path)).size !== requestFields.length ||
    new Set(toolTypes).size !== toolTypes.length ||
    new Set(limitations.map((limitation) => limitation.code)).size !== limitations.length
  ) {
    throw invalid();
  }
  return {
    contractId: readString(value.contract_id),
    requestFields,
    toolTypes,
    limitations,
  };
}

export function readProviderKind(value: unknown): ProviderKind {
  if (
    value !== "openai" &&
    value !== "codex" &&
    value !== "claude" &&
    value !== "grok" &&
    value !== "kimi"
  ) {
    throw invalid();
  }
  return value;
}

export function readProtocolDialect(value: unknown): ProtocolDialect {
  if (
    value !== "openai_responses" &&
    value !== "openai_chat_completions" &&
    value !== "openai_images" &&
    value !== "anthropic_messages"
  ) {
    throw invalid();
  }
  return value;
}

function readProtocolOperation(value: unknown): ProtocolOperation {
  if (
    value !== "responses" &&
    value !== "responses_compact" &&
    value !== "alpha_search" &&
    value !== "chat_completions" &&
    value !== "images_generations" &&
    value !== "images_edits" &&
    value !== "messages" &&
    value !== "messages_count_tokens"
  ) {
    throw invalid();
  }
  return value;
}

function readProtocolFidelity(value: unknown): ProtocolFidelity {
  if (value !== "direct" && value !== "translated") throw invalid();
  return value;
}

function readFieldBehavior(value: unknown): BridgeRequestFieldBehavior {
  if (
    value !== "forwarded" &&
    value !== "translated" &&
    value !== "validated_only" &&
    value !== "local_state"
  ) {
    throw invalid();
  }
  return value;
}

function operationDialect(operation: ProtocolOperation): ProtocolDialect {
  if (
    operation === "responses" ||
    operation === "responses_compact" ||
    operation === "alpha_search"
  ) {
    return "openai_responses";
  }
  if (operation === "chat_completions") return "openai_chat_completions";
  if (operation === "images_generations" || operation === "images_edits") {
    return "openai_images";
  }
  return "anthropic_messages";
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) throw invalid();
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function invalid() {
  return new Error("invalid provider endpoint response");
}
