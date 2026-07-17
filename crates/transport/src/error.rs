use any2api_domain::RetrySafety;
use thiserror::Error;

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

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("transport failed at {stage:?}: {message}")]
pub struct TransportError {
    pub stage: TransportErrorStage,
    pub retry_safety: RetrySafety,
    pub message: String,
}

impl TransportError {
    #[must_use]
    pub fn new(
        stage: TransportErrorStage,
        retry_safety: RetrySafety,
        message: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            retry_safety,
            message: message.into(),
        }
    }
}
