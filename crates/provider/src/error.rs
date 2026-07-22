use any2api_domain::ProviderKind;
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProviderError {
    #[error("provider driver already registered for {0:?}")]
    DuplicateProvider(ProviderKind),
    #[error("invalid provider credential: {0}")]
    InvalidCredential(String),
    #[error("invalid provider endpoint: {0}")]
    InvalidEndpoint(String),
    #[error("invalid provider response: {0}")]
    InvalidResponse(String),
}
