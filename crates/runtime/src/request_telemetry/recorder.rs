use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{
    CompletedRequestLog, ConfigRevision, CredentialId, ErrorClass, GatewayApiKeyId, OAuthAccountId,
    ProtocolOperation, ProviderEndpointId, ProxyProfileId, PublicError, PublicErrorCode,
    RequestAttempt, RequestAttemptOutcome, RequestId, RequestLog, RetrySafety, RouteTargetId,
    TokenUsage, bound_error_message,
};

use super::{RequestLogPolicy, RequestObservation, RequestTelemetry};
use crate::route_candidates::RouteCandidate;

const CANCELLED_STATUS_CODE: u16 = 499;

#[derive(Clone)]
pub(crate) struct RequestRecorder {
    inner: Option<Arc<RequestRecorderInner>>,
}

struct RequestRecorderInner {
    telemetry: Arc<RequestTelemetry>,
    policy: RequestLogPolicy,
    started_at_ms: u64,
    started_at: Instant,
    request_id: RequestId,
    config_revision: ConfigRevision,
    gateway_api_key_id: GatewayApiKeyId,
    operation: ProtocolOperation,
    state: Mutex<RequestRecorderState>,
}

#[derive(Default)]
struct RequestRecorderState {
    public_model: Option<String>,
    is_stream: bool,
    final_target: Option<FinalTarget>,
    attempts: Vec<RequestAttempt>,
    observation: RequestObservation,
    finished: bool,
}

#[derive(Clone, Copy)]
struct FinalTarget {
    endpoint_id: Option<ProviderEndpointId>,
    credential_id: Option<CredentialId>,
    oauth_account_id: Option<OAuthAccountId>,
    proxy_id: ProxyProfileId,
}

impl RequestRecorder {
    pub(crate) fn new(
        telemetry: Arc<RequestTelemetry>,
        policy: RequestLogPolicy,
        request_id: RequestId,
        gateway_api_key_id: GatewayApiKeyId,
        operation: ProtocolOperation,
    ) -> Self {
        if !policy.enabled {
            return Self { inner: None };
        }
        Self {
            inner: Some(Arc::new(RequestRecorderInner {
                telemetry,
                policy,
                started_at_ms: unix_time_ms(),
                started_at: Instant::now(),
                request_id,
                config_revision: policy.revision,
                gateway_api_key_id,
                operation,
                state: Mutex::new(RequestRecorderState::default()),
            })),
        }
    }

    pub(crate) fn set_route(&self, public_model: String, is_stream: bool) {
        let Some(inner) = &self.inner else {
            return;
        };
        let mut state = inner.state.lock().expect("request recorder state");
        state.public_model = Some(public_model);
        state.is_stream = is_stream;
    }

    pub(crate) fn begin_attempt(
        &self,
        attempt_no: u32,
        candidate: &RouteCandidate,
    ) -> AttemptRecorder {
        let Some(inner) = &self.inner else {
            return AttemptRecorder::disabled();
        };
        let target = FinalTarget {
            endpoint_id: candidate
                .credential_id
                .provider_credential_id()
                .map(|_| candidate.endpoint_id),
            credential_id: candidate.credential_id.provider_credential_id(),
            oauth_account_id: candidate.credential_id.oauth_account_id(),
            proxy_id: candidate.proxy_id,
        };
        inner
            .state
            .lock()
            .expect("request recorder state")
            .final_target = Some(target);
        AttemptRecorder {
            request: self.clone(),
            request_id: inner.request_id,
            attempt_no,
            route_target_id: Some(candidate.target_id),
            credential_id: candidate.credential_id.provider_credential_id(),
            oauth_account_id: candidate.credential_id.oauth_account_id(),
            proxy_profile_id: Some(candidate.proxy_id),
            started_at_ms: unix_time_ms(),
            started_at: Instant::now(),
            finished: false,
        }
    }

    pub(crate) fn finish(&self, status_code: u16, error_class: Option<ErrorClass>) {
        self.finish_with_message(status_code, error_class, None);
    }

    pub(crate) fn finish_with_message(
        &self,
        status_code: u16,
        error_class: Option<ErrorClass>,
        error_message: Option<String>,
    ) {
        let Some(inner) = &self.inner else {
            return;
        };
        inner.finish(status_code, error_class, error_message);
    }

    pub(crate) fn finish_public_error(&self, status_code: u16, error: &PublicError) {
        self.finish_with_message(
            status_code,
            Some(public_error_class(error.code)),
            Some(bound_error_message(&error.message)),
        );
    }

    pub(crate) fn observe_token_usage(&self, usage: TokenUsage) {
        let Some(inner) = &self.inner else {
            return;
        };
        let mut state = inner.state.lock().expect("request recorder state");
        if !state.finished {
            state.observation.observe_token_usage(usage);
        }
    }

    pub(crate) fn observe_first_token(&self) {
        let Some(inner) = &self.inner else {
            return;
        };
        let mut state = inner.state.lock().expect("request recorder state");
        if !state.finished {
            state.observation.observe_first_token(inner.started_at);
        }
    }

    fn push_attempt(&self, attempt: RequestAttempt) {
        let Some(inner) = &self.inner else {
            return;
        };
        let mut state = inner.state.lock().expect("request recorder state");
        if !state.finished {
            state.attempts.push(attempt);
        }
    }
}

impl RequestRecorderInner {
    fn finish(
        &self,
        status_code: u16,
        error_class: Option<ErrorClass>,
        error_message: Option<String>,
    ) {
        let record = {
            let mut state = self.state.lock().expect("request recorder state");
            if state.finished {
                return;
            }
            state.finished = true;
            let final_target = state.final_target;
            let observation = state.observation;
            let token_usage = observation.token_usage();
            let attempts = std::mem::take(&mut state.attempts);
            let error_class = final_error_class(&attempts, error_class);
            let error_message = final_error_message(&attempts, error_message);
            CompletedRequestLog {
                request: RequestLog {
                    request_id: self.request_id,
                    started_at_ms: self.started_at_ms,
                    config_revision: self.config_revision,
                    gateway_api_key_id: Some(self.gateway_api_key_id),
                    ingress_protocol: self.operation.dialect(),
                    operation: self.operation,
                    public_model: state.public_model.clone(),
                    provider_endpoint_id: final_target.and_then(|target| target.endpoint_id),
                    credential_id: final_target.and_then(|target| target.credential_id),
                    oauth_account_id: final_target.and_then(|target| target.oauth_account_id),
                    proxy_profile_id: final_target.map(|target| target.proxy_id),
                    status_code,
                    error_class,
                    error_message,
                    attempt_count: u32::try_from(attempts.len()).unwrap_or(u32::MAX),
                    latency_ms: duration_ms(self.started_at.elapsed()),
                    first_token_ms: observation.first_token_ms(),
                    input_tokens: token_usage.input_tokens(),
                    output_tokens: token_usage.output_tokens(),
                    cache_read_tokens: token_usage.cache_read_tokens(),
                    cache_write_tokens: token_usage.cache_write_tokens(),
                    is_stream: state.is_stream,
                },
                attempts,
            }
        };
        self.telemetry.try_record(record, self.policy);
    }
}

impl Drop for RequestRecorderInner {
    fn drop(&mut self) {
        self.finish(
            CANCELLED_STATUS_CODE,
            Some(ErrorClass::Cancelled),
            Some(bound_error_message("request cancelled")),
        );
    }
}

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
    finished: bool,
}

impl AttemptRecorder {
    pub(crate) fn disabled() -> Self {
        Self {
            request: RequestRecorder { inner: None },
            request_id: RequestId::new(),
            attempt_no: 1,
            route_target_id: None,
            credential_id: None,
            oauth_account_id: None,
            proxy_profile_id: None,
            started_at_ms: 0,
            started_at: Instant::now(),
            finished: true,
        }
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
            Some(bound_error_message(message)),
            None,
        );
    }

    pub(crate) fn upstream_error(
        &mut self,
        status_code: u16,
        retry_safety: RetrySafety,
        error_class: ErrorClass,
        message: Option<String>,
    ) {
        self.complete(
            RequestAttemptOutcome::UpstreamError,
            Some(retry_safety),
            Some(error_class),
            message.map(bound_error_message),
            Some(status_code),
        );
    }

    pub(crate) fn invalid_response(&mut self, status_code: Option<u16>, message: impl AsRef<str>) {
        self.complete(
            RequestAttemptOutcome::InvalidResponse,
            Some(RetrySafety::Ambiguous),
            Some(ErrorClass::Upstream),
            Some(bound_error_message(message)),
            status_code,
        );
    }

    pub(crate) fn local_error(
        &mut self,
        status_code: Option<u16>,
        error_class: ErrorClass,
        message: impl AsRef<str>,
    ) {
        self.local_error_with_safety(
            status_code,
            error_class,
            RetrySafety::Ambiguous,
            message,
        );
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
            Some(bound_error_message(message)),
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
            Some(bound_error_message(message)),
            Some(status_code),
        );
    }

    pub(crate) fn cancelled(&mut self, status_code: Option<u16>) {
        self.complete(
            RequestAttemptOutcome::Cancelled,
            Some(RetrySafety::Ambiguous),
            Some(ErrorClass::Cancelled),
            Some(bound_error_message("request cancelled")),
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
        let error_message = error_message.and_then(|message| {
            let trimmed = message.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        });
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

pub(crate) const fn public_error_class(code: PublicErrorCode) -> ErrorClass {
    match code {
        PublicErrorCode::Unauthorized => ErrorClass::Authentication,
        PublicErrorCode::InvalidRequest
        | PublicErrorCode::PublicApiNotFound
        | PublicErrorCode::MethodNotAllowed
        | PublicErrorCode::ModelNotFound
        | PublicErrorCode::NoRoute => ErrorClass::InvalidRequest,
        PublicErrorCode::UpstreamNotFound => ErrorClass::OperationUnavailable,
        PublicErrorCode::InternalError => ErrorClass::Internal,
        PublicErrorCode::NoAvailableCredential
        | PublicErrorCode::LocalConcurrencyLimit
        | PublicErrorCode::SessionBindingLost
        | PublicErrorCode::UpstreamError => ErrorClass::Upstream,
    }
}

fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn final_error_class(
    attempts: &[RequestAttempt],
    fallback: Option<ErrorClass>,
) -> Option<ErrorClass> {
    match (
        attempts.last().and_then(|attempt| attempt.error_class),
        fallback,
    ) {
        (Some(ErrorClass::Cancelled), Some(fallback)) if fallback != ErrorClass::Cancelled => {
            Some(fallback)
        }
        (Some(error_class), _) => Some(error_class),
        (None, fallback) => fallback,
    }
}

fn final_error_message(
    attempts: &[RequestAttempt],
    fallback: Option<String>,
) -> Option<String> {
    // Prefer the public/client-visible message when finish provides one.
    // Fall back to the last attempt diagnostic for stream/drop paths.
    match (
        attempts
            .last()
            .and_then(|attempt| attempt.error_message.clone()),
        fallback,
    ) {
        (_, Some(message)) => Some(message),
        (Some(message), None) => Some(message),
        (None, None) => None,
    }
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
