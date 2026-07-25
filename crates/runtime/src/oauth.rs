mod activation;
mod callback;
mod document;
mod error;
mod import;
mod quota;
mod quota_request;
mod quota_types;
pub(crate) mod refresh;
mod service;
mod session;
mod token_request;
mod types;

#[cfg(test)]
#[path = "oauth_tests.rs"]
mod oauth_tests;

#[cfg(test)]
#[path = "oauth/refresh_tests.rs"]
mod refresh_tests;

#[cfg(test)]
#[path = "oauth/quota_tests.rs"]
mod quota_tests;

#[cfg(test)]
#[path = "oauth/import_tests.rs"]
mod import_tests;

pub use error::OAuthError;
pub use import::{
    MAX_OAUTH_IMPORT_ACCOUNTS, OAuthImportError, OAuthImportFailureKind, OAuthImportResult,
};
pub use quota_types::{OAuthQuotaError, OAuthQuotaResetOutcome, OAuthQuotaSnapshot};
pub use service::OAuthService;
pub use types::{OAuthActivationResult, OAuthDevicePollResult, OAuthStartFlow, OAuthStartResult};
