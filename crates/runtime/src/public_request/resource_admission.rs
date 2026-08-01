use std::{
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use any2api_domain::{ProtocolOperation, PublicError, PublicErrorCode};
use any2api_protocol::api::{IngressAffinity, MAX_BRIDGE_CONTINUATION_STATE_BYTES};
use bytes::Bytes;
use futures_util::Stream;
use thiserror::Error;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::{PublicResponse, PublicResponseBody, PublicResponseStream, execution_limits};

/// Process-wide budget reserved for public-request payload working sets.
/// This is deliberately fixed: it is a safety boundary, not a scheduler knob.
pub const PUBLIC_REQUEST_MEMORY_BUDGET_BYTES: usize = 256 * 1024 * 1024;

const MEMORY_UNIT_BYTES: usize = 64 * 1024;
const REQUEST_PARSE_COPIES: usize = 4;
const RESPONSE_TRANSFORM_COPIES: usize = 3;

pub(super) struct PublicRequestMemoryBudget {
    semaphore: Arc<Semaphore>,
}

impl PublicRequestMemoryBudget {
    pub(super) fn new() -> Self {
        Self::with_capacity_bytes(PUBLIC_REQUEST_MEMORY_BUDGET_BYTES)
    }

    fn with_capacity_bytes(bytes: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(units_for(bytes))),
        }
    }

    pub(super) fn try_admit_max_body(
        &self,
        operation: ProtocolOperation,
    ) -> Result<PublicRequestMemoryAdmission, PublicRequestAdmissionError> {
        self.try_admit_body(operation, execution_limits::request_body_limit(operation))
    }

    pub(super) fn try_admit_body(
        &self,
        operation: ProtocolOperation,
        body_bytes: usize,
    ) -> Result<PublicRequestMemoryAdmission, PublicRequestAdmissionError> {
        validate_body_size(operation, body_bytes)?;
        let permit = try_acquire(&self.semaphore, request_parse_bytes(body_bytes))?;
        Ok(PublicRequestMemoryAdmission {
            operation,
            body_bytes,
            response_lifetime_bytes: 0,
            permit,
        })
    }
}

pub struct PublicRequestMemoryAdmission {
    operation: ProtocolOperation,
    body_bytes: usize,
    response_lifetime_bytes: usize,
    permit: OwnedSemaphorePermit,
}

impl PublicRequestMemoryAdmission {
    pub async fn run_blocking<T, F>(self, work: F) -> Result<(T, Self), tokio::task::JoinError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        tokio::task::spawn_blocking(move || (work(), self)).await
    }

    pub fn set_body_len(&mut self, body_bytes: usize) -> Result<(), PublicRequestAdmissionError> {
        validate_body_size(self.operation, body_bytes)?;
        self.resize(request_parse_bytes(body_bytes))?;
        self.body_bytes = body_bytes;
        Ok(())
    }

    pub(super) fn validate_operation(
        &self,
        operation: ProtocolOperation,
    ) -> Result<(), PublicRequestAdmissionError> {
        if self.operation == operation {
            Ok(())
        } else {
            Err(PublicRequestAdmissionError::OperationMismatch)
        }
    }

    pub(super) fn reserve_execution(
        &mut self,
        response_limit_bytes: usize,
        affinity: &IngressAffinity,
    ) -> Result<(), PublicRequestAdmissionError> {
        let request_peak = request_parse_bytes(self.body_bytes);
        let response_lifetime_bytes = continuation_working_set_bytes(affinity);
        let response_peak = self
            .body_bytes
            .saturating_add(response_limit_bytes.saturating_mul(RESPONSE_TRANSFORM_COPIES))
            .saturating_add(response_lifetime_bytes);
        self.resize(request_peak.max(response_peak))?;
        self.response_lifetime_bytes = response_lifetime_bytes;
        Ok(())
    }

    fn resize(&mut self, bytes: usize) -> Result<(), PublicRequestAdmissionError> {
        let target = units_for(bytes);
        let current = self.permit.num_permits();
        if target > current {
            let added = try_acquire(
                self.permit.semaphore(),
                (target - current) * MEMORY_UNIT_BYTES,
            )?;
            self.permit.merge(added);
        } else if current > target {
            drop(
                self.permit
                    .split(current - target)
                    .expect("admission owns the released permits"),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PublicRequestAdmissionError {
    #[error("public request memory budget is exhausted")]
    Capacity,
    #[error("public request body exceeds its endpoint limit")]
    PayloadTooLarge,
    #[error("public request memory admission operation does not match the request")]
    OperationMismatch,
}

pub(super) fn public_error(error: PublicRequestAdmissionError) -> PublicError {
    match error {
        PublicRequestAdmissionError::Capacity => PublicError::new(
            PublicErrorCode::LocalRateLimit,
            "server request memory budget is exhausted",
        )
        .with_retry_after_seconds(1),
        PublicRequestAdmissionError::PayloadTooLarge => PublicError::new(
            PublicErrorCode::PayloadTooLarge,
            "request body exceeds the public API request size limit",
        ),
        PublicRequestAdmissionError::OperationMismatch => PublicError::new(
            PublicErrorCode::InternalError,
            "request resource admission did not match the endpoint",
        ),
    }
}

pub(super) fn hold_response_memory(
    mut admission: PublicRequestMemoryAdmission,
    mut response: PublicResponse,
) -> PublicResponse {
    response.body = match response.body {
        PublicResponseBody::Buffered(body) => {
            admission
                .resize(body.len().saturating_add(admission.response_lifetime_bytes))
                .expect("a response is smaller than its reserved working set");
            PublicResponseBody::Buffered(Bytes::from_owner(AdmittedBytes {
                body,
                _admission: admission,
            }))
        }
        PublicResponseBody::Streaming(inner) => {
            PublicResponseBody::Streaming(Box::pin(AdmittedStream {
                inner,
                admission: Some(admission),
            }))
        }
    };
    response
}

fn validate_body_size(
    operation: ProtocolOperation,
    body_bytes: usize,
) -> Result<(), PublicRequestAdmissionError> {
    if body_bytes > execution_limits::request_body_limit(operation) {
        Err(PublicRequestAdmissionError::PayloadTooLarge)
    } else {
        Ok(())
    }
}

fn request_parse_bytes(body_bytes: usize) -> usize {
    body_bytes.saturating_mul(REQUEST_PARSE_COPIES)
}

fn continuation_working_set_bytes(affinity: &IngressAffinity) -> usize {
    if matches!(affinity, IngressAffinity::Continuation(_)) {
        MAX_BRIDGE_CONTINUATION_STATE_BYTES
    } else {
        0
    }
}

fn units_for(bytes: usize) -> usize {
    (bytes.saturating_add(MEMORY_UNIT_BYTES - 1) / MEMORY_UNIT_BYTES).max(1)
}

fn try_acquire(
    semaphore: &Arc<Semaphore>,
    bytes: usize,
) -> Result<OwnedSemaphorePermit, PublicRequestAdmissionError> {
    let permits =
        u32::try_from(units_for(bytes)).map_err(|_| PublicRequestAdmissionError::Capacity)?;
    Arc::clone(semaphore)
        .try_acquire_many_owned(permits)
        .map_err(|_| PublicRequestAdmissionError::Capacity)
}

struct AdmittedBytes {
    body: Bytes,
    _admission: PublicRequestMemoryAdmission,
}

impl AsRef<[u8]> for AdmittedBytes {
    fn as_ref(&self) -> &[u8] {
        self.body.as_ref()
    }
}

struct AdmittedStream {
    inner: PublicResponseStream,
    admission: Option<PublicRequestMemoryAdmission>,
}

impl Stream for AdmittedStream {
    type Item = Result<Bytes, io::Error>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let result = this.inner.as_mut().poll_next(context);
        if matches!(result, Poll::Ready(None) | Poll::Ready(Some(Err(_)))) {
            this.admission.take();
        }
        result
    }
}

#[cfg(test)]
mod tests;
