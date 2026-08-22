//! Cross-crate contract test package.

mod admin_session;
mod gateway_authentication;
mod test_application;

pub use admin_session::TestAdminSession;
pub use any2api::PublicRequestComponents;
pub use gateway_authentication::{TestGatewayAuthentication, create_gateway_authentication};
pub use test_application::TestApplication;

use std::sync::Arc;

use any2api_domain::ProviderKind;
use any2api_provider::api::{OfficialClientVersion, ProviderRegistry};
use any2api_runtime::api::RequestTelemetry;

pub fn build_public_request_components() -> anyhow::Result<PublicRequestComponents> {
    let components = any2api::build_public_request_components()?;
    seed_official_client_versions(components.provider_registry());
    Ok(components)
}

pub fn build_public_request_components_with_telemetry(
    telemetry: Arc<RequestTelemetry>,
) -> anyhow::Result<PublicRequestComponents> {
    let components = any2api::build_public_request_components_with_telemetry(telemetry)?;
    seed_official_client_versions(components.provider_registry());
    Ok(components)
}

pub fn seed_official_client_versions(providers: &ProviderRegistry) {
    for (provider, version) in [
        (ProviderKind::Codex, "0.145.0"),
        (ProviderKind::Claude, "2.1.220"),
        (ProviderKind::Grok, "0.2.112"),
    ] {
        let Some(versioned) = providers
            .get(provider)
            .and_then(|driver| driver.official_client_version())
        else {
            continue;
        };
        versioned.publish_official_client_version(
            OfficialClientVersion::new(version).expect("valid test official client version"),
        );
    }
}

pub fn build_configuration_capabilities()
-> std::sync::Arc<any2api_runtime::api::ConfigurationCapabilities> {
    build_public_request_components()
        .expect("public request components")
        .configuration_capabilities()
}

pub fn build_provider_registry() -> std::sync::Arc<any2api_provider::api::ProviderRegistry> {
    build_public_request_components()
        .expect("public request components")
        .provider_registry_handle()
}
