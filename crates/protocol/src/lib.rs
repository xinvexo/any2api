pub mod api;

mod affinity;
mod anthropic_messages;
mod error;
mod json_codec;
mod openai_chat_completions;
mod openai_responses;
mod openai_responses_chat;
mod registry;
mod sse;
mod telemetry;

pub use anthropic_messages::AnthropicMessagesAdapter;
pub use error::ProtocolError;
pub use openai_chat_completions::OpenAiChatCompletionsAdapter;
pub use openai_responses::OpenAiResponsesAdapter;
pub use openai_responses_chat::ResponsesToChatCompletionsBridge;
pub use registry::ProtocolRegistry;
pub use sse::SseDecoder;
