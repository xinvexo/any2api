mod activation;
mod callback;
mod document;
mod error;
mod import;
mod quota;
mod quota_observation;
mod quota_request;
mod quota_types;
pub(crate) mod refresh;
mod service;
mod session;
mod token_request;
mod types;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod refresh_tests;

#[cfg(test)]
mod quota_tests;

#[cfg(test)]
mod quota_test_support;

#[cfg(test)]
mod quota_mock_transport;

#[cfg(test)]
mod grok_quota_tests;

#[cfg(test)]
mod claude_quota_tests;

#[cfg(test)]
mod import_tests;

pub use error::OAuthError;
pub use import::{
    MAX_OAUTH_IMPORT_ACCOUNTS, OAuthImportError, OAuthImportFailureKind, OAuthImportResult,
};
pub use quota_types::{OAuthQuotaError, OAuthQuotaResetOutcome, OAuthQuotaSnapshot};
pub use service::OAuthService;
pub use types::{OAuthActivationResult, OAuthDevicePollResult, OAuthStartFlow, OAuthStartResult};
