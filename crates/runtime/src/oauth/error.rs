use any2api_domain::ProviderKind;
use any2api_provider::api::ProviderError;
use any2api_transport::api::TransportError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("provider driver is unavailable")]
    ProviderUnavailable,
    #[error("OAuth2 is not supported by {0:?}")]
    UnsupportedProvider(ProviderKind),
    #[error("too many OAuth sessions are active")]
    SessionCapacity,
    #[error("OAuth session is invalid or was already used")]
    InvalidSession,
    #[error("OAuth session expired")]
    SessionExpired,
    #[error("OAuth callback URL is invalid")]
    InvalidCallback,
    #[error("OAuth authorization was denied")]
    AuthorizationDenied,
    #[error("OAuth state does not match the session")]
    StateMismatch,
    #[error("operating system randomness is unavailable")]
    RandomGeneration,
    #[error("OAuth provider request is invalid: {0}")]
    Provider(#[from] ProviderError),
    #[error("OAuth token request failed: {0}")]
    Transport(#[from] TransportError),
    #[error("OAuth token endpoint rejected the request with status {0}")]
    TokenRejected(u16),
    #[error("OAuth token response read timed out")]
    TokenReadTimeout,
    #[error("OAuth token response exceeded the size limit")]
    TokenResponseTooLarge,
    #[error("OAuth provider returned an invalid token response")]
    TokenResponseInvalid,
    #[error("OAuth authentication file could not be generated")]
    FileSerialization,
}

impl OAuthError {
    pub(super) fn from_token_response_error(error: ProviderError) -> Self {
        match error {
            ProviderError::InvalidResponse(_) => Self::TokenResponseInvalid,
            error => Self::Provider(error),
        }
    }
}
