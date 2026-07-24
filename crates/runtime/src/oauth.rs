mod callback;
mod document;
mod error;
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

pub use error::OAuthError;
pub use quota_types::{OAuthQuotaError, OAuthQuotaResetOutcome, OAuthQuotaSnapshot};
pub use service::OAuthService;
pub use types::{OAuthActivationResult, OAuthStartResult};
