use std::sync::Arc;

use any2api_domain::ConfigRevision;
use any2api_protocol::{
    AnthropicMessagesAdapter, OpenAiChatCompletionsAdapter, OpenAiImagesAdapter,
    OpenAiResponsesAdapter, ProtocolRegistry, ResponsesToChatCompletionsBridge,
};
use any2api_provider::{ClaudeDriver, CodexDriver, GrokDriver, ProviderRegistry};
use any2api_storage::api::{
    ConfigurationMutation, ConfigurationRepository, SqliteStore, StorageError, StoredConfiguration,
};

use crate::configuration::ConfigurationCapabilities;

pub(crate) fn configuration_capabilities() -> Arc<ConfigurationCapabilities> {
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("Responses adapter");
    protocols
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("Chat Completions adapter");
    protocols
        .register(Arc::new(OpenAiImagesAdapter::new()))
        .expect("Images adapter");
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
    providers
        .register(Arc::new(GrokDriver::new()))
        .expect("Grok driver");

    Arc::new(ConfigurationCapabilities::new(
        Arc::new(protocols),
        Arc::new(providers),
    ))
}

pub(crate) async fn commit_configuration(
    store: &SqliteStore,
    expected: ConfigRevision,
    mutation: ConfigurationMutation,
) -> Result<StoredConfiguration, StorageError> {
    let prepared = store.prepare_configuration(expected, mutation).await?;
    let (candidate, commit) = prepared.into_parts();
    commit.finish().await?;
    Ok(candidate)
}
