mod api_key_secret;
mod auth;
mod model_catalog;
mod model_probe;
mod runtime;

pub use api_key_secret::ProviderApiKeySecret;
#[cfg(test)]
pub(crate) use auth::CredentialAuthMaterial;
pub(crate) use auth::{CredentialAuthMaterialError, CredentialAuthMaterials};
pub(crate) use model_catalog::{ModelCatalogReadError, collect as collect_model_catalog};
pub use model_probe::{
    ProviderCredentialTestError, ProviderCredentialTestFailureScope,
    ProviderCredentialTestFailureStage, ProviderCredentialTestOutcome,
    ProviderCredentialTestResult, ProviderCredentialTestService,
};
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
