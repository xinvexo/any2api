use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Codex,
    Claude,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolDialect {
    #[serde(rename = "openai_responses", alias = "open_ai_responses")]
    OpenAiResponses,
    CodexBackend,
    AnthropicMessages,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolOperation {
    Responses,
    ResponsesCompact,
    Messages,
    MessagesCountTokens,
}

impl ProtocolOperation {
    pub const ALL: [Self; 4] = [
        Self::Responses,
        Self::ResponsesCompact,
        Self::Messages,
        Self::MessagesCountTokens,
    ];

    #[must_use]
    pub const fn dialect(self) -> ProtocolDialect {
        match self {
            Self::Responses | Self::ResponsesCompact => ProtocolDialect::OpenAiResponses,
            Self::Messages | Self::MessagesCountTokens => ProtocolDialect::AnthropicMessages,
        }
    }

    #[must_use]
    pub const fn allows_stream(self) -> bool {
        matches!(self, Self::Responses | Self::Messages)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportMode {
    Json,
    Sse,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    ApiKey,
}
