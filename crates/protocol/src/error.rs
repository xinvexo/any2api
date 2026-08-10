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
    #[error("protocol bridge continuation state is too large: {bytes} > {max_bytes} bytes")]
    ContinuationTooLarge { bytes: usize, max_bytes: usize },
    #[error("protocol processing failed: {0}")]
    Internal(String),
    #[error("invalid protocol payload: {0}")]
    InvalidPayload(String),
}

impl ProtocolError {
    pub(crate) fn unsupported_field(scope: Option<&str>, field: &str) -> Self {
        let message = match (scope, diagnostic_token(field)) {
            (Some(scope), Some(field)) => format!("unsupported field `{scope}.{field}`"),
            (Some(scope), None) => format!("unsupported field under `{scope}`"),
            (None, Some(field)) => format!("unsupported field `{field}`"),
            (None, None) => "request contains an unsupported field".into(),
        };
        Self::InvalidPayload(message)
    }

    pub(crate) fn unsupported_value(label: &str, value: &str) -> Self {
        let message = diagnostic_token(value).map_or_else(
            || format!("{label} is unsupported"),
            |value| format!("{label} `{value}` is unsupported"),
        );
        Self::InvalidPayload(message)
    }
}

fn diagnostic_token(value: &str) -> Option<&str> {
    const MAX_BYTES: usize = 64;

    (!value.is_empty()
        && value.len() <= MAX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')))
    .then_some(value)
}

#[cfg(test)]
mod tests {
    use super::ProtocolError;

    #[test]
    fn unsupported_diagnostics_only_echo_safe_bounded_tokens() {
        assert_eq!(
            ProtocolError::unsupported_field(Some("text"), "future"),
            ProtocolError::InvalidPayload("unsupported field `text.future`".into())
        );
        assert_eq!(
            ProtocolError::unsupported_value("tool type", "computer\nsecret"),
            ProtocolError::InvalidPayload("tool type is unsupported".into())
        );
    }
}
