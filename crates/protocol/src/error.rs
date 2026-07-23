use any2api_domain::ProtocolDialect;
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProtocolError {
    #[error("protocol adapter already registered for {0:?}")]
    DuplicateDialect(ProtocolDialect),
    #[error("protocol bridge already registered for {0:?} -> {1:?}")]
    DuplicateBridge(ProtocolDialect, ProtocolDialect),
    #[error("unsupported protocol operation: {0}")]
    Unsupported(String),
    #[error("protocol bridge session binding was lost")]
    SessionBindingLost,
    #[error("invalid protocol payload: {0}")]
    InvalidPayload(String),
}
