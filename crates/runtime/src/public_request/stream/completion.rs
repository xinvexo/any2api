use std::time::Instant;

use any2api_domain::{
    ANY2API_UPSTREAM_TIMEOUT_MESSAGE, ErrorClass, PublicError, PublicErrorCode, UpstreamError,
};
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
        self.continuation_lease.take();
        self.precommit_continuation.take();
        if self.owns_request_completion
            && matches!(&outcome, StreamOutcome::Cancelled)
            && let Some(recorder) = self.attempt_recorder.as_mut()
        {
            recorder.observe_stream_cancel();
        }
        if let Some(health) = self.health.take() {
            match &outcome {
                StreamOutcome::Success => health.success(),
                StreamOutcome::UpstreamFailure(error) => {
                    health.upstream_failure(error.classification());
                }
                StreamOutcome::Error { .. } | StreamOutcome::Cancelled => {}
            }
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            match &outcome {
                StreamOutcome::Success => recorder.success(self.status_code),
                StreamOutcome::UpstreamFailure(error) => {
                    let classification = error.classification();
                    recorder.upstream_error(
                        self.status_code,
                        classification.retry_safety(),
                        classification.kind().error_class(),
                        error.official_message(),
                    );
                }
                StreamOutcome::Error { class, message } => {
                    recorder.stream_error(*class, self.status_code, message);
                }
                StreamOutcome::Cancelled => recorder.cancelled(Some(self.status_code)),
            }
        }
        if self.owns_request_completion {
            match outcome {
                StreamOutcome::Success => self.request_recorder.finish(self.status_code, None),
                StreamOutcome::UpstreamFailure(error) => {
                    let classification = error.classification();
                    self.request_recorder.finish_with_message(
                        self.status_code,
                        Some(classification.kind().error_class()),
                        Some(
                            error
                                .official_message()
                                .unwrap_or("upstream response stream reported a failure event")
                                .to_owned(),
                        ),
                    );
                }
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
        self.quota_activity.take();
    }

    pub(super) fn release_guards(&mut self) {
        self.state = super::CommitState::Finished;
        self.cancellation.cancel();
        self.upstream = Box::pin(futures_util::stream::empty());
        self.continuation_lease.take();
        self.precommit_continuation.take();
        self.health.take();
        self.permit.take();
        self.quota_activity.take();
    }

    pub(in crate::public_request) fn commit_precommit_continuation(
        &mut self,
        deadline: Option<Instant>,
    ) -> Result<(), PublicError> {
        if let Err(error) = self.finalize_precommit_continuation(deadline) {
            self.set_pending_error(error);
            return Err(self.finish_precommit_failure());
        }
        Ok(())
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
            PendingStreamErrorKind::Timeout => {
                self.health.take();
                if let Some(mut recorder) = self.attempt_recorder.take() {
                    recorder.local_error(Some(self.status_code), ErrorClass::Network, &diagnostic);
                }
            }
            PendingStreamErrorKind::Local => {
                if let Some(health) = self.health.take() {
                    health.success();
                }
                if let Some(mut recorder) = self.attempt_recorder.take() {
                    recorder.local_error(Some(self.status_code), ErrorClass::Internal, &diagnostic);
                }
            }
        }
        self.release_guards();
        match kind {
            PendingStreamErrorKind::Timeout => public_error(
                PublicErrorCode::GatewayTimeout,
                ANY2API_UPSTREAM_TIMEOUT_MESSAGE,
            ),
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

    pub(super) fn finish_precommit_upstream_failure(&mut self, error: &UpstreamError) {
        let classification = error.classification();
        if let Some(health) = self.health.take() {
            health.upstream_failure(classification);
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.upstream_error(
                self.status_code,
                classification.retry_safety(),
                classification.kind().error_class(),
                error.official_message(),
            );
        }
        self.release_guards();
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
