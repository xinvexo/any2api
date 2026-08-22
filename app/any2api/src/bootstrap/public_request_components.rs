use std::sync::Arc;

use any2api_protocol::api::ProtocolRegistry;
use any2api_protocol::{
    AnthropicMessagesAdapter, ImagesToChatCompletionsBridge, OpenAiChatCompletionsAdapter,
    OpenAiImagesAdapter, OpenAiResponsesAdapter, ResponsesToChatCompletionsBridge,
};
use any2api_provider::{
    ClaudeDriver, CodexDriver, GrokDriver, KimiDriver, OpenAiDriver, api::ProviderRegistry,
};
use any2api_runtime::api::{
    ConfigurationCapabilities, ProviderCredentialTestService, ProxyTestService,
    PublicRequestService, RequestTelemetry,
};
use any2api_transport::api::{ReqwestTransportManager, TransportManager, TransportManagerConfig};

pub struct PublicRequestComponents {
    protocols: Arc<ProtocolRegistry>,
    providers: Arc<ProviderRegistry>,
    configuration_capabilities: Arc<ConfigurationCapabilities>,
    transport: Arc<dyn TransportManager>,
    telemetry: Arc<RequestTelemetry>,
    service: Arc<PublicRequestService>,
    proxy_tests: Arc<ProxyTestService>,
    provider_credential_tests: Arc<ProviderCredentialTestService>,
}

impl PublicRequestComponents {
    #[must_use]
    pub fn protocol_registry(&self) -> &ProtocolRegistry {
        self.protocols.as_ref()
    }

    #[must_use]
    pub fn provider_registry(&self) -> &ProviderRegistry {
        self.providers.as_ref()
    }

    #[must_use]
    pub fn provider_registry_handle(&self) -> Arc<ProviderRegistry> {
        Arc::clone(&self.providers)
    }

    #[must_use]
    pub fn configuration_capabilities(&self) -> Arc<ConfigurationCapabilities> {
        Arc::clone(&self.configuration_capabilities)
    }

    #[must_use]
    pub fn transport_manager(&self) -> Arc<dyn TransportManager> {
        Arc::clone(&self.transport)
    }

    #[must_use]
    pub fn service(&self) -> Arc<PublicRequestService> {
        Arc::clone(&self.service)
    }

    pub fn service_with_oauth(
        &self,
        oauth: &any2api_runtime::api::OAuthService,
    ) -> Result<Arc<PublicRequestService>, any2api_runtime::api::PublicRequestServiceError> {
        PublicRequestService::new_with_oauth(
            Arc::clone(&self.protocols),
            Arc::clone(&self.providers),
            Arc::clone(&self.transport),
            oauth,
        )
        .map(|service| Arc::new(service.with_telemetry(Arc::clone(&self.telemetry))))
    }

    #[must_use]
    pub fn proxy_test_service(&self) -> Arc<ProxyTestService> {
        Arc::clone(&self.proxy_tests)
    }

    #[must_use]
    pub fn provider_credential_test_service(&self) -> Arc<ProviderCredentialTestService> {
        Arc::clone(&self.provider_credential_tests)
    }
}

pub fn build_public_request_components() -> anyhow::Result<PublicRequestComponents> {
    build_public_request_components_with_telemetry(Arc::new(RequestTelemetry::disabled()))
}

pub fn build_public_request_components_with_telemetry(
    telemetry: Arc<RequestTelemetry>,
) -> anyhow::Result<PublicRequestComponents> {
    let mut protocols = ProtocolRegistry::new();
    protocols.register(Arc::new(OpenAiResponsesAdapter::new()))?;
    protocols.register(Arc::new(OpenAiChatCompletionsAdapter::new()))?;
    protocols.register(Arc::new(OpenAiImagesAdapter::new()))?;
    protocols.register(Arc::new(AnthropicMessagesAdapter::new()))?;
    protocols.register_bridge(Arc::new(ResponsesToChatCompletionsBridge::new()))?;
    protocols.register_bridge(Arc::new(ImagesToChatCompletionsBridge::new()))?;
    let protocols = Arc::new(protocols);

    let mut providers = ProviderRegistry::new();
    providers.register(Arc::new(OpenAiDriver::new()))?;
    providers.register(Arc::new(CodexDriver::new()))?;
    providers.register(Arc::new(ClaudeDriver::new()))?;
    providers.register(Arc::new(GrokDriver::new()))?;
    providers.register(Arc::new(KimiDriver::new()))?;
    let providers = Arc::new(providers);
    let configuration_capabilities = Arc::new(ConfigurationCapabilities::new(
        Arc::clone(&protocols),
        Arc::clone(&providers),
    ));

    let transport: Arc<dyn TransportManager> = Arc::new(ReqwestTransportManager::new(
        TransportManagerConfig::default(),
    )?);
    let proxy_tests = Arc::new(ProxyTestService::new(Arc::clone(&transport)));
    let provider_credential_tests = Arc::new(ProviderCredentialTestService::new(
        Arc::clone(&providers),
        Arc::clone(&transport),
    ));
    let service = Arc::new(
        PublicRequestService::new(
            Arc::clone(&protocols),
            Arc::clone(&providers),
            Arc::clone(&transport),
        )?
        .with_telemetry(Arc::clone(&telemetry)),
    );
    Ok(PublicRequestComponents {
        protocols,
        providers,
        configuration_capabilities,
        transport,
        telemetry,
        service,
        proxy_tests,
        provider_credential_tests,
    })
}
