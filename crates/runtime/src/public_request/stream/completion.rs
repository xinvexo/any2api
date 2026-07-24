use any2api_domain::{ErrorClass, PublicError, PublicErrorCode};
use any2api_transport::api::{TransportError, TransportFailureScope};

use super::{
    GuardedBody, StreamOutcome,
    pending_failure::{PendingStreamError, PendingStreamErrorKind, transport_error_class},
};
use crate::public_request::response::public_error;

impl GuardedBody {
    pub(super) fn finish(&mut self, outcome: StreamOutcome) {
        if self.state == super::CommitState::Finished {
            return;
        }
        self.state = super::CommitState::Finished;
        self.cancellation.cancel();
        self.upstream = Box::pin(futures_util::stream::empty());
        self.health.take();
        if let Some(mut recorder) = self.attempt_recorder.take() {
            match &outcome {
                StreamOutcome::Success => recorder.success(self.status_code),
                StreamOutcome::Error { class, message } => {
                    recorder.stream_error(*class, self.status_code, message);
                }
                StreamOutcome::Cancelled => recorder.cancelled(Some(self.status_code)),
            }
        }
        if self.owns_request_completion {
            match outcome {
                StreamOutcome::Success => self.request_recorder.finish(self.status_code, None),
                StreamOutcome::Error { class, message } => {
                    self.request_recorder.finish_with_message(
                        self.status_code,
                        Some(class),
                        Some(message),
                    );
                }
                StreamOutcome::Cancelled => self.request_recorder.finish_with_message(
                    self.status_code,
                    Some(ErrorClass::Cancelled),
                    Some("request cancelled".to_owned()),
                ),
            }
        }
        self.permit.take();
    }

    pub(super) fn release_guards(&mut self) {
        self.state = super::CommitState::Finished;
        self.cancellation.cancel();
        self.upstream = Box::pin(futures_util::stream::empty());
        self.health.take();
        self.permit.take();
    }

    pub(super) fn finish_precommit_failure(&mut self) -> PublicError {
        let pending = self.pending_error.take();
        let kind = pending
            .as_ref()
            .map_or(PendingStreamErrorKind::InvalidResponse, |error| error.kind);
        let diagnostic = pending
            .as_ref()
            .map(PendingStreamError::message)
            .unwrap_or_else(|| "upstream stream ended before the first event".to_owned());
        match kind {
            PendingStreamErrorKind::Transport {
                retry_safety,
                failure_scope,
            } => {
                if let Some(health) = self.health.take() {
                    health.transport_failure(failure_scope);
                }
                if let Some(mut recorder) = self.attempt_recorder.take() {
                    recorder.transport_error(
                        retry_safety,
                        transport_error_class(failure_scope),
                        &diagnostic,
                    );
                }
            }
            PendingStreamErrorKind::InvalidResponse => {
                if let Some(health) = self.health.take() {
                    health.transport_failure(TransportFailureScope::Endpoint);
                }
                if let Some(mut recorder) = self.attempt_recorder.take() {
                    recorder.invalid_response(Some(self.status_code), &diagnostic);
                }
            }
            PendingStreamErrorKind::BudgetExceeded => {
                if let Some(health) = self.health.take() {
                    health.success();
                }
                if let Some(mut recorder) = self.attempt_recorder.take() {
                    recorder.invalid_response(Some(self.status_code), &diagnostic);
                }
            }
            PendingStreamErrorKind::Local => {
                if let Some(health) = self.health.take() {
                    health.success();
                }
                if let Some(mut recorder) = self.attempt_recorder.take() {
                    recorder.local_error(
                        Some(self.status_code),
                        ErrorClass::Internal,
                        &diagnostic,
                    );
                }
            }
        }
        self.release_guards();
        match kind {
            PendingStreamErrorKind::Local => public_error(
                PublicErrorCode::InternalError,
                "internal stream processing failed",
            ),
            PendingStreamErrorKind::Transport { .. }
            | PendingStreamErrorKind::InvalidResponse
            | PendingStreamErrorKind::BudgetExceeded => public_error(
                PublicErrorCode::UpstreamError,
                if pending.is_some() {
                    "upstream stream failed before the first event"
                } else {
                    "upstream stream ended before the first event"
                },
            ),
        }
    }

    pub(super) fn set_pending_error(&mut self, error: PendingStreamError) {
        if self.pending_error.is_none() {
            self.pending_error = Some(error);
        }
    }

    pub(super) fn set_transport_error(&mut self, error: &TransportError) {
        self.set_pending_error(PendingStreamError::transport(error));
    }

    pub(super) fn set_timeout_error(&mut self) {
        self.set_pending_error(PendingStreamError::timeout());
    }

    pub(super) fn set_postcommit_idle_timeout_error(&mut self) {
        self.set_pending_error(PendingStreamError::postcommit_idle_timeout());
    }
}
