mod api_key_secret;
mod auth;
mod model_catalog;
mod runtime;
mod test;
#[cfg(test)]
mod test_tests;

pub use api_key_secret::ProviderApiKeySecret;
#[cfg(test)]
pub(crate) use auth::CredentialAuthMaterial;
pub(crate) use auth::CredentialAuthMaterials;
pub(crate) use model_catalog::{ModelCatalogReadError, collect as collect_model_catalog};
#[cfg(test)]
pub(crate) use runtime::CredentialRuntimeBindings;
pub(crate) use runtime::{
    CredentialAuthentication, CredentialFilterKind, CredentialGenerationDefinition,
    CredentialRuntimeHandle, RateLimited,
};
pub use runtime::{
    CredentialBalancingCounters, CredentialGenerationRuntime, CredentialRateSnapshot,
    CredentialRuntimeBinding, RoutingPermit,
};
pub use test::{
    ProviderCredentialTestError, ProviderCredentialTestFailureScope,
    ProviderCredentialTestFailureStage, ProviderCredentialTestOutcome,
    ProviderCredentialTestResult, ProviderCredentialTestService,
};
