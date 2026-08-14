use any2api_domain::{ProtocolDialect, ProtocolOperation, ProviderKind};
use any2api_protocol::api::{BridgeRequestFieldBehavior, ProtocolFidelity};

use super::{ConfigurationCapabilityError, ProviderProtocolOptions};

#[test]
fn options_are_derived_from_registered_bridges_and_provider_capabilities() {
    let capabilities = crate::test_support::configuration_capabilities();
    let codex = capabilities.provider_protocol_options(ProviderKind::Codex);

    assert_eq!(codex.len(), 3);
    assert_eq!(codex[0].accepted_protocol, ProtocolDialect::OpenAiResponses);
    assert_eq!(
        upstream_protocols(&codex[0]),
        [
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
        ]
    );
    let direct = &codex[0].upstream_options[0];
    assert_eq!(direct.fidelity, ProtocolFidelity::Direct);
    assert_eq!(
        direct.operations,
        [
            ProtocolOperation::Responses,
            ProtocolOperation::ResponsesCompact,
            ProtocolOperation::AlphaSearch
        ]
    );
    assert!(direct.bridge.is_none());

    let translated = &codex[0].upstream_options[1];
    assert_eq!(translated.fidelity, ProtocolFidelity::Translated);
    assert_eq!(translated.operations, [ProtocolOperation::Responses]);
    let bridge = translated.bridge.expect("Responses bridge contract");
    assert_eq!(
        bridge.contract_id,
        "openai-responses-to-chat-completions/v1"
    );
    assert!(bridge.supports_tool_type("function"));
    assert_eq!(
        bridge
            .request_field("client_metadata")
            .map(|field| field.behavior),
        Some(BridgeRequestFieldBehavior::ValidatedOnly)
    );
    assert!(bridge.request_field("future_field").is_none());

    assert_eq!(
        upstream_protocols(&codex[1]),
        [ProtocolDialect::OpenAiChatCompletions]
    );
    assert_eq!(codex[2].accepted_protocol, ProtocolDialect::OpenAiImages);
    assert_eq!(
        upstream_protocols(&codex[2]),
        [
            ProtocolDialect::OpenAiChatCompletions,
            ProtocolDialect::OpenAiImages,
        ]
    );

    let grok = capabilities.provider_protocol_options(ProviderKind::Grok);
    assert_eq!(grok.len(), 3);
    assert_eq!(grok[0].accepted_protocol, ProtocolDialect::OpenAiResponses);
    assert_eq!(
        upstream_protocols(&grok[0]),
        [
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
        ]
    );
    assert_eq!(grok[2].accepted_protocol, ProtocolDialect::OpenAiImages);
    assert_eq!(
        upstream_protocols(&grok[2]),
        [ProtocolDialect::OpenAiChatCompletions]
    );

    let kimi = capabilities.provider_protocol_options(ProviderKind::Kimi);
    assert_eq!(kimi.len(), 3);
    assert_eq!(kimi[0].accepted_protocol, ProtocolDialect::OpenAiResponses);
    assert_eq!(
        upstream_protocols(&kimi[0]),
        [ProtocolDialect::OpenAiChatCompletions]
    );
    assert_eq!(
        kimi[0].upstream_options[0].fidelity,
        ProtocolFidelity::Translated
    );
    assert_eq!(
        upstream_protocols(&kimi[1]),
        [ProtocolDialect::OpenAiChatCompletions]
    );
}

#[test]
fn endpoint_validation_uses_the_registered_pair_and_provider_driver() {
    let capabilities = crate::test_support::configuration_capabilities();
    capabilities
        .validate_endpoint(
            ProviderKind::Codex,
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
        )
        .expect("registered bridge");
    capabilities
        .validate_endpoint(
            ProviderKind::Codex,
            ProtocolDialect::OpenAiImages,
            ProtocolDialect::OpenAiImages,
        )
        .expect("registered Images adapter and Codex capability");
    capabilities
        .validate_endpoint(
            ProviderKind::Codex,
            ProtocolDialect::OpenAiImages,
            ProtocolDialect::OpenAiChatCompletions,
        )
        .expect("registered Images to Chat Completions bridge");
    capabilities
        .validate_endpoint(
            ProviderKind::Grok,
            ProtocolDialect::OpenAiImages,
            ProtocolDialect::OpenAiChatCompletions,
        )
        .expect("bridge options are derived without a Provider-specific branch");
    capabilities
        .validate_endpoint(
            ProviderKind::Kimi,
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiChatCompletions,
        )
        .expect("Kimi uses the registered Responses to Chat bridge");

    assert!(matches!(
        capabilities.validate_endpoint(
            ProviderKind::Codex,
            ProtocolDialect::AnthropicMessages,
            ProtocolDialect::OpenAiResponses,
        ),
        Err(ConfigurationCapabilityError::MissingProtocolBridge { .. })
    ));
    assert!(matches!(
        capabilities.validate_endpoint(
            ProviderKind::Claude,
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiResponses,
        ),
        Err(ConfigurationCapabilityError::UnsupportedProviderProtocol { .. })
    ));
    assert!(matches!(
        capabilities.validate_endpoint(
            ProviderKind::Grok,
            ProtocolDialect::OpenAiImages,
            ProtocolDialect::OpenAiImages,
        ),
        Err(ConfigurationCapabilityError::UnsupportedProviderProtocol { .. })
    ));
    assert!(matches!(
        capabilities.validate_endpoint(
            ProviderKind::Kimi,
            ProtocolDialect::OpenAiResponses,
            ProtocolDialect::OpenAiResponses,
        ),
        Err(ConfigurationCapabilityError::UnsupportedProviderProtocol { .. })
    ));
}

fn upstream_protocols(options: &ProviderProtocolOptions) -> Vec<ProtocolDialect> {
    options
        .upstream_options
        .iter()
        .map(|option| option.protocol)
        .collect()
}
