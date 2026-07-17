use any2api_domain::ProtocolDialect;
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProtocolError {
    #[error("protocol adapter already registered for {0:?}")]
    DuplicateDialect(ProtocolDialect),
    #[error("unsupported protocol operation: {0}")]
    Unsupported(String),
    #[error("invalid protocol payload: {0}")]
    InvalidPayload(String),
}
