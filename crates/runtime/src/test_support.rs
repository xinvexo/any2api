use std::sync::Arc;

use any2api_protocol::{
    AnthropicMessagesAdapter, OpenAiChatCompletionsAdapter, OpenAiResponsesAdapter,
    ProtocolRegistry, ResponsesToChatCompletionsBridge,
};
use any2api_provider::{ClaudeDriver, CodexDriver, ProviderRegistry};

use crate::configuration_capabilities::ConfigurationCapabilities;

pub(crate) fn configuration_capabilities() -> Arc<ConfigurationCapabilities> {
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("Responses adapter");
    protocols
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("Chat Completions adapter");
    protocols
        .register(Arc::new(AnthropicMessagesAdapter::new()))
        .expect("Messages adapter");
    protocols
        .register_bridge(Arc::new(ResponsesToChatCompletionsBridge::new()))
        .expect("Responses to Chat Completions bridge");

    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    providers
        .register(Arc::new(ClaudeDriver::new()))
        .expect("Claude driver");

    Arc::new(ConfigurationCapabilities::new(
        Arc::new(protocols),
        Arc::new(providers),
    ))
}
