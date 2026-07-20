use std::sync::Arc;

use any2api_protocol::{AnthropicMessagesAdapter, OpenAiResponsesAdapter, ProtocolRegistry};
use any2api_provider::{ClaudeDriver, CodexDriver, ProviderRegistry};
use any2api_runtime::api::{PublicRequestService, RequestTelemetry};
use any2api_transport::api::{ReqwestTransportManager, TransportManager, TransportManagerConfig};

pub struct PublicRequestComponents {
    protocols: Arc<ProtocolRegistry>,
    providers: Arc<ProviderRegistry>,
    service: Arc<PublicRequestService>,
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
    pub fn service(&self) -> Arc<PublicRequestService> {
        Arc::clone(&self.service)
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
    protocols.register(Arc::new(AnthropicMessagesAdapter::new()))?;
    let protocols = Arc::new(protocols);

    let mut providers = ProviderRegistry::new();
    providers.register(Arc::new(CodexDriver::new()))?;
    providers.register(Arc::new(ClaudeDriver::new()))?;
    let providers = Arc::new(providers);

    let transport: Arc<dyn TransportManager> = Arc::new(ReqwestTransportManager::new(
        TransportManagerConfig::default(),
    )?);
    let service = Arc::new(
        PublicRequestService::new(Arc::clone(&protocols), Arc::clone(&providers), transport)?
            .with_telemetry(telemetry),
    );
    Ok(PublicRequestComponents {
        protocols,
        providers,
        service,
    })
}
