use std::{fmt, sync::Arc};

use any2api_domain::{ProtocolDialect, ProtocolOperation};

use super::{DecodedRequest, StartedProtocolBridge};
use crate::ProtocolError;

/// Hard per-entry limit for protocol-owned continuation state. Runtime also
/// reserves this amount for every pending bridged stream.
pub const MAX_BRIDGE_CONTINUATION_STATE_BYTES: usize = 16 * 1024 * 1024;

pub(crate) trait ResumableProtocolContinuation: Send + Sync {
    fn ingress_dialect(&self) -> ProtocolDialect;
    fn upstream_dialect(&self) -> ProtocolDialect;
    fn operation(&self) -> ProtocolOperation;
    fn serialized_bytes(&self) -> usize;

    fn resume(
        &self,
        request: &DecodedRequest,
        upstream_model: &str,
    ) -> Result<StartedProtocolBridge, ProtocolError>;
}

/// Opaque, cloneable protocol capability stored by Runtime without downcasts.
#[derive(Clone)]
pub struct ProtocolContinuationState {
    inner: Arc<dyn ResumableProtocolContinuation>,
    serialized_bytes: usize,
}

impl ProtocolContinuationState {
    pub(crate) fn new(
        inner: Arc<dyn ResumableProtocolContinuation>,
    ) -> Result<Self, ProtocolError> {
        let serialized_bytes = inner.serialized_bytes();
        if serialized_bytes > MAX_BRIDGE_CONTINUATION_STATE_BYTES {
            return Err(ProtocolError::ContinuationTooLarge {
                bytes: serialized_bytes,
                max_bytes: MAX_BRIDGE_CONTINUATION_STATE_BYTES,
            });
        }
        Ok(Self {
            inner,
            serialized_bytes,
        })
    }

    #[must_use]
    pub const fn serialized_bytes(&self) -> usize {
        self.serialized_bytes
    }

    pub(crate) fn resume(
        &self,
        ingress: ProtocolDialect,
        upstream: ProtocolDialect,
        operation: ProtocolOperation,
        request: &DecodedRequest,
        upstream_model: &str,
    ) -> Result<StartedProtocolBridge, ProtocolError> {
        if self.inner.ingress_dialect() != ingress
            || self.inner.upstream_dialect() != upstream
            || self.inner.operation() != operation
        {
            return Err(ProtocolError::SessionBindingLost);
        }
        self.inner.resume(request, upstream_model)
    }
}

impl fmt::Debug for ProtocolContinuationState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtocolContinuationState")
            .field("ingress_dialect", &self.inner.ingress_dialect())
            .field("upstream_dialect", &self.inner.upstream_dialect())
            .field("operation", &self.inner.operation())
            .field("serialized_bytes", &self.serialized_bytes)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub enum BridgeContinuationState {
    Stateless,
    Pending,
    Ready(ProtocolContinuationState),
}
