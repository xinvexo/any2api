use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use any2api_domain::{
    ANY2API_UPSTREAM_TIMEOUT_MESSAGE, CredentialId, ErrorClass, OAuthAccountId, ProxyProfileId,
    RequestAttempt, RequestAttemptOutcome, RequestId, RetrySafety, RouteTargetId, TokenUsage,
};

use super::request::{RequestRecorder, bound_optional_error_message, duration_ms};
use crate::routing::RouteCandidate;

pub(crate) struct AttemptRecorder {
    request: RequestRecorder,
    request_id: RequestId,
    attempt_no: u32,
    route_target_id: Option<RouteTargetId>,
    credential_id: Option<CredentialId>,
    oauth_account_id: Option<OAuthAccountId>,
    proxy_profile_id: Option<ProxyProfileId>,
    started_at_ms: u64,
    started_at: Instant,
    timeout: AttemptTimeoutMarker,
    finished: bool,
}

#[derive(Clone)]
pub(crate) struct AttemptTimeoutMarker(Arc<AtomicBool>);

impl AttemptTimeoutMarker {
    pub(crate) fn mark_timed_out(&self) {
        self.0.store(true, Ordering::Release);
    }

    fn timed_out(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl AttemptRecorder {
    pub(super) fn new(
        request: RequestRecorder,
        request_id: RequestId,
        attempt_no: u32,
        candidate: &RouteCandidate,
        started_at_ms: u64,
    ) -> Self {
        Self {
            request,
            request_id,
            attempt_no,
            route_target_id: Some(candidate.target_id),
            credential_id: candidate.credential_id.provider_credential_id(),
            oauth_account_id: candidate.credential_id.oauth_account_id(),
            proxy_profile_id: Some(candidate.proxy_id),
            started_at_ms,
            started_at: Instant::now(),
            timeout: AttemptTimeoutMarker(Arc::new(AtomicBool::new(false))),
            finished: false,
        }
    }

    pub(crate) fn disabled() -> Self {
        Self {
            request: RequestRecorder::disabled(),
            request_id: RequestId::new(),
            attempt_no: 1,
            route_target_id: None,
            credential_id: None,
            oauth_account_id: None,
            proxy_profile_id: None,
            started_at_ms: 0,
            started_at: Instant::now(),
            timeout: AttemptTimeoutMarker(Arc::new(AtomicBool::new(false))),
            finished: true,
        }
    }

    pub(crate) fn timeout_marker(&self) -> AttemptTimeoutMarker {
        self.timeout.clone()
    }

    pub(crate) fn request(&self) -> RequestRecorder {
        self.request.clone()
    }

    pub(crate) fn observe_token_usage(&self, usage: TokenUsage) {
        self.request.observe_token_usage(usage);
    }

    pub(crate) fn success(&mut self, status_code: u16) {
        self.complete(
            RequestAttemptOutcome::Success,
            None,
            None,
            None,
            Some(status_code),
        );
    }

    pub(crate) fn transport_error(
        &mut self,
        retry_safety: RetrySafety,
        error_class: ErrorClass,
        message: impl AsRef<str>,
    ) {
        self.complete(
            RequestAttemptOutcome::TransportError,
            Some(retry_safety),
            Some(error_class),
            bound_optional_error_message(message),
            None,
        );
    }

    pub(crate) fn upstream_error(
        &mut self,
        status_code: u16,
        retry_safety: RetrySafety,
        error_class: ErrorClass,
        message: Option<&str>,
    ) {
        self.complete(
            RequestAttemptOutcome::UpstreamError,
            Some(retry_safety),
            Some(error_class),
            message.and_then(bound_optional_error_message),
            Some(status_code),
        );
    }

    pub(crate) fn invalid_response(&mut self, status_code: Option<u16>, message: impl AsRef<str>) {
        self.complete(
            RequestAttemptOutcome::InvalidResponse,
            Some(RetrySafety::Ambiguous),
            Some(ErrorClass::Upstream),
            bound_optional_error_message(message),
            status_code,
        );
    }

    pub(crate) fn local_error(
        &mut self,
        status_code: Option<u16>,
        error_class: ErrorClass,
        message: impl AsRef<str>,
    ) {
        self.local_error_with_safety(status_code, error_class, RetrySafety::Ambiguous, message);
    }

    pub(crate) fn local_error_before_send(
        &mut self,
        status_code: Option<u16>,
        error_class: ErrorClass,
        message: impl AsRef<str>,
    ) {
        self.local_error_with_safety(
            status_code,
            error_class,
            RetrySafety::DefinitelyNotSent,
            message,
        );
    }

    fn local_error_with_safety(
        &mut self,
        status_code: Option<u16>,
        error_class: ErrorClass,
        retry_safety: RetrySafety,
        message: impl AsRef<str>,
    ) {
        self.complete(
            RequestAttemptOutcome::LocalError,
            Some(retry_safety),
            Some(error_class),
            bound_optional_error_message(message),
            status_code,
        );
    }

    pub(crate) fn stream_error(
        &mut self,
        error_class: ErrorClass,
        status_code: u16,
        message: impl AsRef<str>,
    ) {
        self.complete(
            RequestAttemptOutcome::StreamError,
            Some(RetrySafety::Ambiguous),
            Some(error_class),
            bound_optional_error_message(message),
            Some(status_code),
        );
    }

    pub(crate) fn stream_rejected(&mut self, status_code: u16) {
        self.complete(
            RequestAttemptOutcome::UpstreamError,
            Some(RetrySafety::RejectedBeforeExecution),
            Some(ErrorClass::Upstream),
            bound_optional_error_message("upstream stream was rejected before content"),
            Some(status_code),
        );
    }

    pub(crate) fn cancelled(&mut self, status_code: Option<u16>) {
        if self.timeout.timed_out() {
            self.complete(
                RequestAttemptOutcome::LocalError,
                Some(RetrySafety::Ambiguous),
                Some(ErrorClass::Network),
                bound_optional_error_message(ANY2API_UPSTREAM_TIMEOUT_MESSAGE),
                None,
            );
            return;
        }
        self.complete(
            RequestAttemptOutcome::Cancelled,
            Some(RetrySafety::Ambiguous),
            Some(ErrorClass::Cancelled),
            bound_optional_error_message("request cancelled"),
            status_code,
        );
    }

    fn complete(
        &mut self,
        outcome: RequestAttemptOutcome,
        retry_safety: Option<RetrySafety>,
        error_class: Option<ErrorClass>,
        error_message: Option<String>,
        status_code: Option<u16>,
    ) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.request.push_attempt(RequestAttempt {
            request_id: self.request_id,
            attempt_no: self.attempt_no,
            route_target_id: self.route_target_id,
            credential_id: self.credential_id,
            oauth_account_id: self.oauth_account_id,
            proxy_profile_id: self.proxy_profile_id,
            started_at_ms: self.started_at_ms,
            duration_ms: duration_ms(self.started_at.elapsed()),
            retry_safety,
            error_class,
            error_message,
            status_code,
            outcome,
        });
    }
}

impl Drop for AttemptRecorder {
    fn drop(&mut self) {
        self.cancelled(None);
    }
}
