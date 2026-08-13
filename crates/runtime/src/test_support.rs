use std::{convert::Infallible, sync::Arc};

use any2api_domain::ConfigRevision;
use any2api_protocol::{
    AnthropicMessagesAdapter, ImagesToChatCompletionsBridge, OpenAiChatCompletionsAdapter,
    OpenAiImagesAdapter, OpenAiResponsesAdapter, ResponsesToChatCompletionsBridge,
    api::ProtocolRegistry,
};
use any2api_provider::{ClaudeDriver, CodexDriver, GrokDriver, KimiDriver, api::ProviderRegistry};
use any2api_storage::api::{
    ConfigurationMutation, ConfigurationRepository, ConfigurationTransactionOutcome,
    ConfigurationTransactionRepository, SqliteStore, StorageError, StoredConfiguration,
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
    protocols
        .register_bridge(Arc::new(ImagesToChatCompletionsBridge::new()))
        .expect("Images to Chat Completions bridge");

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
    providers
        .register(Arc::new(KimiDriver::new()))
        .expect("Kimi driver");

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
    let outcome = <SqliteStore as ConfigurationTransactionRepository<
        StoredConfiguration,
        Infallible,
    >>::transact_configuration(store, expected, mutation, Box::new(Ok))
    .await?;
    match outcome {
        ConfigurationTransactionOutcome::NoChange => store.load_configuration().await,
        ConfigurationTransactionOutcome::Committed(configuration) => Ok(configuration),
        ConfigurationTransactionOutcome::Rejected(never) => match never {},
    }
}
