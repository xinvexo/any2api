//! Cross-crate contract test package.

pub use any2api::{
    PublicRequestComponents, build_public_request_components,
    build_public_request_components_with_telemetry,
};

pub fn build_configuration_capabilities()
-> std::sync::Arc<any2api_runtime::api::ConfigurationCapabilities> {
    build_public_request_components()
        .expect("public request components")
        .configuration_capabilities()
}
