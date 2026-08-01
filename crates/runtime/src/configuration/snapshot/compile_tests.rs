use any2api_domain::{
    ConfigRevision, GatewayApiKeyConfiguration, ModelRouteConfiguration, OAuthAccountConfiguration,
    ProtocolDialect, ProviderCredentialConfiguration, ProviderEndpoint,
    ProviderEndpointConfiguration, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyConfiguration, SettingsConfiguration,
};
use any2api_provider::api::ProviderRegistry;
use any2api_storage::api::{
    GatewayApiKeyVerifier, StoredConfiguration, StoredOAuthAccountMaterials,
    StoredProviderCredentialSecrets, StoredProxyPasswords,
};

use super::PreparedPublishedSnapshot;
use crate::registry::RuntimeRegistry;

#[test]
fn compiling_a_candidate_does_not_mutate_the_runtime_registry() {
    let runtime = RuntimeRegistry::new();
    let providers = crate::test_support::configuration_capabilities();

    let prepared = PreparedPublishedSnapshot::compile(
        stored_configuration(ProviderEndpointConfiguration::initial()),
        providers.provider_registry(),
    )
    .expect("valid candidate");

    assert_eq!(runtime.active_credential_count(), 0);
    let snapshot = prepared.bind(&runtime);
    assert_eq!(snapshot.revision(), ConfigRevision::INITIAL);
    assert_eq!(runtime.active_credential_count(), 0);
}

#[test]
fn compiling_rejects_an_unregistered_provider_before_binding() {
    let endpoint = ProviderEndpoint::create(
        ProviderEndpointId::new(),
        ProviderEndpointDraft::new(
            "Codex",
            ProviderKind::Codex,
            "https://api.openai.com/v1",
            ProtocolDialect::OpenAiResponses,
            true,
        )
        .expect("endpoint draft"),
    )
    .expect("endpoint");
    let endpoints = ProviderEndpointConfiguration::new(vec![endpoint]).expect("configuration");

    let error = match PreparedPublishedSnapshot::compile(
        stored_configuration(endpoints),
        &ProviderRegistry::new(),
    ) {
        Ok(_) => panic!("missing driver must reject the candidate"),
        Err(error) => error,
    };

    assert_eq!(
        error.to_string(),
        "routing credential material is inconsistent: provider driver is not registered: Codex"
    );
}

fn stored_configuration(endpoints: ProviderEndpointConfiguration) -> StoredConfiguration {
    StoredConfiguration::new(
        ConfigRevision::INITIAL,
        ProxyConfiguration::initial(),
        endpoints,
        ProviderCredentialConfiguration::initial(),
        OAuthAccountConfiguration::initial(),
        ModelRouteConfiguration::default(),
        GatewayApiKeyConfiguration::initial(),
        GatewayApiKeyVerifier::new(),
        SettingsConfiguration::defaults(),
        StoredProviderCredentialSecrets::default(),
        StoredOAuthAccountMaterials::default(),
        StoredProxyPasswords::default(),
    )
}
