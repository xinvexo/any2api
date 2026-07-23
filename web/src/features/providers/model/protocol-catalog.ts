import type { ProtocolDialect } from "../api/provider-contracts";

export function protocolLabel(protocol: ProtocolDialect): string {
  switch (protocol) {
    case "openai_responses":
      return "OpenAI Responses";
    case "openai_chat_completions":
      return "OpenAI Chat Completions";
    case "codex_backend":
      return "Codex Backend";
    case "anthropic_messages":
      return "Anthropic Messages";
  }
}
