pub mod api;

mod affinity;
mod anthropic_messages;
mod error;
mod json_codec;
mod openai_chat_completions;
mod openai_images;
mod openai_images_chat;
mod openai_responses;
mod openai_responses_chat;
mod openai_responses_websocket;
mod raw_json;
mod registry;
mod sse;
mod stream_rejection;
mod telemetry;

pub use anthropic_messages::AnthropicMessagesAdapter;
pub(crate) use error::ProtocolError;
pub use openai_chat_completions::OpenAiChatCompletionsAdapter;
pub use openai_images::OpenAiImagesAdapter;
pub use openai_images_chat::ImagesToChatCompletionsBridge;
pub use openai_responses::OpenAiResponsesAdapter;
pub use openai_responses_chat::ResponsesToChatCompletionsBridge;
