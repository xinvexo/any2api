mod call;
mod definition;
mod name;
mod state;

use crate::api::{OpenAiChatCompletionsProfile, OpenAiChatCustomToolMode};

pub(super) use call::{ProjectedSourceCall, RestoredToolCall};
pub(super) use state::ToolProjection;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ChatToolKind {
    Function,
    Custom,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ToolIdentity {
    Function { name: String },
    Custom { name: String },
    NamespaceFunction { namespace: String, name: String },
    NamespaceCustom { namespace: String, name: String },
    ToolSearch,
}

impl ToolIdentity {
    pub(super) const fn source_call_kind(&self) -> SourceCallKind {
        match self {
            Self::Function { .. } | Self::NamespaceFunction { .. } => SourceCallKind::Function,
            Self::Custom { .. } | Self::NamespaceCustom { .. } => SourceCallKind::Custom,
            Self::ToolSearch => SourceCallKind::ToolSearch,
        }
    }

    fn chat_kind(&self, profile: OpenAiChatCompletionsProfile) -> ChatToolKind {
        match self {
            Self::Custom { .. } | Self::NamespaceCustom { .. }
                if profile.custom_tools == OpenAiChatCustomToolMode::Native =>
            {
                ChatToolKind::Custom
            }
            _ => ChatToolKind::Function,
        }
    }

    fn diagnostic_name(&self) -> &str {
        match self {
            Self::Function { name }
            | Self::Custom { name }
            | Self::NamespaceFunction { name, .. }
            | Self::NamespaceCustom { name, .. } => name,
            Self::ToolSearch => "tool_search",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SourceCallKind {
    Function,
    Custom,
    ToolSearch,
}
