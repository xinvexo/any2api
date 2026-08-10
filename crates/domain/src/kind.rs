use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Codex,
    Claude,
    Grok,
    Kimi,
}

impl ProviderKind {
    pub const ALL: [Self; 4] = [Self::Codex, Self::Claude, Self::Grok, Self::Kimi];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Grok => "grok",
            Self::Kimi => "kimi",
        }
    }

    #[must_use]
    pub const fn supports_oauth(self) -> bool {
        matches!(self, Self::Codex | Self::Claude | Self::Grok)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolDialect {
    #[serde(rename = "openai_responses")]
    OpenAiResponses,
    #[serde(rename = "openai_chat_completions")]
    OpenAiChatCompletions,
    #[serde(rename = "openai_images")]
    OpenAiImages,
    AnthropicMessages,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolOperation {
    Responses,
    ResponsesCompact,
    ChatCompletions,
    ImagesGenerations,
    ImagesEdits,
    Messages,
    MessagesCountTokens,
}

impl ProtocolOperation {
    pub const ALL: [Self; 7] = [
        Self::Responses,
        Self::ResponsesCompact,
        Self::ChatCompletions,
        Self::ImagesGenerations,
        Self::ImagesEdits,
        Self::Messages,
        Self::MessagesCountTokens,
    ];

    #[must_use]
    pub const fn dialect(self) -> ProtocolDialect {
        match self {
            Self::Responses | Self::ResponsesCompact => ProtocolDialect::OpenAiResponses,
            Self::ChatCompletions => ProtocolDialect::OpenAiChatCompletions,
            Self::ImagesGenerations | Self::ImagesEdits => ProtocolDialect::OpenAiImages,
            Self::Messages | Self::MessagesCountTokens => ProtocolDialect::AnthropicMessages,
        }
    }

    #[must_use]
    pub const fn allows_stream(self) -> bool {
        matches!(
            self,
            Self::Responses
                | Self::ChatCompletions
                | Self::ImagesGenerations
                | Self::ImagesEdits
                | Self::Messages
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportMode {
    Json,
    Sse,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum RequestBodyEncoding {
    #[default]
    Identity,
    Zstd,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialKind {
    ApiKey,
}

impl ProtocolDialect {
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "OpenAI Responses",
            Self::OpenAiChatCompletions => "Chat Completions",
            Self::OpenAiImages => "OpenAI Images",
            Self::AnthropicMessages => "Anthropic Messages",
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai_responses",
            Self::OpenAiChatCompletions => "openai_chat_completions",
            Self::OpenAiImages => "openai_images",
            Self::AnthropicMessages => "anthropic_messages",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "openai_responses" => Some(Self::OpenAiResponses),
            "openai_chat_completions" => Some(Self::OpenAiChatCompletions),
            "openai_images" => Some(Self::OpenAiImages),
            "anthropic_messages" => Some(Self::AnthropicMessages),
            _ => None,
        }
    }
}

impl ProtocolOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ResponsesCompact => "responses_compact",
            Self::ChatCompletions => "chat_completions",
            Self::ImagesGenerations => "images_generations",
            Self::ImagesEdits => "images_edits",
            Self::Messages => "messages",
            Self::MessagesCountTokens => "messages_count_tokens",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "responses" => Some(Self::Responses),
            "responses_compact" => Some(Self::ResponsesCompact),
            "chat_completions" => Some(Self::ChatCompletions),
            "images_generations" => Some(Self::ImagesGenerations),
            "images_edits" => Some(Self::ImagesEdits),
            "messages" => Some(Self::Messages),
            "messages_count_tokens" => Some(Self::MessagesCountTokens),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProtocolDialect, ProtocolOperation, ProviderKind};

    #[test]
    fn provider_values_and_oauth_support_are_stable() {
        assert_eq!(ProviderKind::ALL.len(), 4);
        assert_eq!(ProviderKind::Kimi.as_str(), "kimi");
        assert!(!ProviderKind::Kimi.supports_oauth());
        assert_eq!(
            serde_json::to_string(&ProviderKind::Kimi).expect("serialize provider"),
            r#""kimi""#
        );
        assert_eq!(
            serde_json::from_str::<ProviderKind>(r#""kimi""#).expect("deserialize provider"),
            ProviderKind::Kimi
        );
    }

    #[test]
    fn protocol_display_names_are_stable() {
        assert_eq!(
            ProtocolDialect::OpenAiResponses.display_name(),
            "OpenAI Responses"
        );
        assert_eq!(
            ProtocolDialect::OpenAiChatCompletions.display_name(),
            "Chat Completions"
        );
    }

    #[test]
    fn image_protocol_values_are_stable() {
        assert_eq!(ProtocolDialect::OpenAiImages.as_str(), "openai_images");
        assert_eq!(
            ProtocolDialect::parse("openai_images"),
            Some(ProtocolDialect::OpenAiImages)
        );
        assert_eq!(
            serde_json::to_string(&ProtocolDialect::OpenAiImages).expect("serialize dialect"),
            r#""openai_images""#
        );
        assert_eq!(
            serde_json::from_str::<ProtocolDialect>(r#""openai_images""#)
                .expect("deserialize dialect"),
            ProtocolDialect::OpenAiImages
        );
        assert_eq!(
            ProtocolOperation::ImagesGenerations.as_str(),
            "images_generations"
        );
        assert_eq!(
            ProtocolOperation::parse("images_generations"),
            Some(ProtocolOperation::ImagesGenerations)
        );
        assert_eq!(ProtocolOperation::ImagesEdits.as_str(), "images_edits");
        assert_eq!(
            ProtocolOperation::parse("images_edits"),
            Some(ProtocolOperation::ImagesEdits)
        );
        assert_eq!(
            serde_json::to_string(&ProtocolOperation::ImagesEdits).expect("serialize operation"),
            r#""images_edits""#
        );
    }

    #[test]
    fn image_operations_use_the_images_dialect_and_allow_streaming() {
        for operation in [
            ProtocolOperation::ImagesGenerations,
            ProtocolOperation::ImagesEdits,
        ] {
            assert_eq!(operation.dialect(), ProtocolDialect::OpenAiImages);
            assert!(operation.allows_stream());
            assert!(ProtocolOperation::ALL.contains(&operation));
        }
    }
}
