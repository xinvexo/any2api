use any2api_domain::RetrySafety;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum TransportConfigurationError {
    #[error("transport client cache capacity must be greater than zero")]
    EmptyClientCache,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportErrorStage {
    Dns,
    Tcp,
    ProxyHandshake,
    Tls,
    WriteRequest,
    AwaitHeaders,
    ReadBody,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportFailureScope {
    Endpoint,
    Proxy,
    Unattributed,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("transport failed at {stage:?}: {message}")]
pub struct TransportError {
    pub stage: TransportErrorStage,
    pub failure_scope: TransportFailureScope,
    pub retry_safety: RetrySafety,
    pub message: String,
}

impl TransportError {
    #[must_use]
    pub fn new(
        stage: TransportErrorStage,
        failure_scope: TransportFailureScope,
        retry_safety: RetrySafety,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            failure_scope,
            retry_safety,
            message: message.into(),
        }
    }

    pub(crate) fn proxy_unavailable(message: impl Into<String>) -> Self {
        Self::new(
            TransportErrorStage::ProxyHandshake,
            TransportFailureScope::Proxy,
            RetrySafety::DefinitelyNotSent,
            message,
        )
    }
}
