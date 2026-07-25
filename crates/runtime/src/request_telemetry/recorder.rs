use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{
    CompletedRequestLog, ConfigRevision, CredentialId, ErrorClass, GatewayApiKeyId, OAuthAccountId,
    ProtocolOperation, ProviderEndpointId, ProxyProfileId, PublicError, PublicErrorCode,
    RequestAttempt, RequestId, RequestLog, TokenUsage, bound_error_message,
};

use super::{RequestLogPolicy, RequestObservation, RequestTelemetry};
use crate::route_candidates::RouteCandidate;

const CANCELLED_STATUS_CODE: u16 = 499;

mod attempt;

pub(crate) use attempt::AttemptRecorder;

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
        AttemptRecorder::new(
            self.clone(),
            inner.request_id,
            attempt_no,
            candidate,
            unix_time_ms(),
        )
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
        inner.finish(
            status_code,
            error_class,
            error_message.and_then(bound_optional_error_message),
        );
    }

    pub(crate) fn finish_public_error(&self, status_code: u16, error: &PublicError) {
        self.finish_with_message(
            status_code,
            Some(public_error_class(error.code)),
            bound_optional_error_message(&error.message),
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

pub(crate) const fn public_error_class(code: PublicErrorCode) -> ErrorClass {
    match code {
        PublicErrorCode::Unauthorized => ErrorClass::Authentication,
        PublicErrorCode::InvalidRequest
        | PublicErrorCode::PublicApiNotFound
        | PublicErrorCode::MethodNotAllowed
        | PublicErrorCode::ModelNotFound
        | PublicErrorCode::NoRoute => ErrorClass::InvalidRequest,
        PublicErrorCode::UpstreamNotFound => ErrorClass::OperationUnavailable,
        PublicErrorCode::LocalRateLimit => ErrorClass::RateLimited,
        PublicErrorCode::InternalError => ErrorClass::Internal,
        PublicErrorCode::NoAvailableCredential
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

fn bound_optional_error_message(message: impl AsRef<str>) -> Option<String> {
    let bounded = bound_error_message(message);
    (!bounded.is_empty()).then_some(bounded)
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

fn final_error_message(attempts: &[RequestAttempt], fallback: Option<String>) -> Option<String> {
    // Prefer the public/client-visible message when finish provides one.
    // Fall back to the last attempt diagnostic for stream/drop paths.
    let selected = match (
        attempts
            .last()
            .and_then(|attempt| attempt.error_message.clone()),
        fallback,
    ) {
        (_, Some(message)) => Some(message),
        (Some(message), None) => Some(message),
        (None, None) => None,
    };
    selected.and_then(bound_optional_error_message)
}

#[cfg(test)]
#[path = "recorder_tests.rs"]
mod tests;
