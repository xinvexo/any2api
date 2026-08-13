use std::{
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{
    ConfigRevision, CredentialId, ErrorClass, GatewayApiKeyId, OAuthAccountId, ProtocolOperation,
    ProviderEndpointId, ProxyProfileId, PublicErrorCode, RequestAttempt,
    RequestAttemptFailureScope, RequestAttemptRetryDecision, RequestId, RequestQuotaCostRate,
    TokenUsage, bound_error_message,
};

use super::super::{RequestLogPolicy, RequestObservation, RequestTelemetry};
use super::attempt::AttemptRecorder;
use crate::routing::RouteCandidate;

pub(super) const CANCELLED_STATUS_CODE: u16 = 499;

#[derive(Clone)]
pub(crate) struct RequestRecorder {
    inner: Option<Arc<RequestRecorderInner>>,
}

pub(super) struct RequestRecorderInner {
    pub(super) telemetry: Arc<RequestTelemetry>,
    pub(super) policy: RequestLogPolicy,
    pub(super) started_at_ms: u64,
    pub(super) started_at: Instant,
    pub(super) request_id: RequestId,
    pub(super) config_revision: ConfigRevision,
    pub(super) gateway_api_key_id: GatewayApiKeyId,
    pub(super) client_ip: IpAddr,
    pub(super) operation: ProtocolOperation,
    pub(super) state: Mutex<RequestRecorderState>,
}

#[derive(Default)]
pub(super) struct RequestRecorderState {
    pub(super) public_model: Option<String>,
    pub(super) thinking_level: Option<String>,
    pub(super) is_stream: bool,
    pub(super) final_target: Option<FinalTarget>,
    pub(super) attempts: Vec<RequestAttempt>,
    pub(super) observation: RequestObservation,
    pub(super) quota_cost_rate: Option<RequestQuotaCostRate>,
    pub(super) finished: bool,
}

#[derive(Clone, Copy)]
pub(super) struct FinalTarget {
    pub(super) endpoint_id: Option<ProviderEndpointId>,
    pub(super) credential_id: Option<CredentialId>,
    pub(super) oauth_account_id: Option<OAuthAccountId>,
    pub(super) proxy_id: ProxyProfileId,
}

impl RequestRecorder {
    pub(super) const fn disabled() -> Self {
        Self { inner: None }
    }

    pub(crate) fn new(
        telemetry: Arc<RequestTelemetry>,
        policy: RequestLogPolicy,
        request_id: RequestId,
        gateway_api_key_id: GatewayApiKeyId,
        client_ip: IpAddr,
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
                client_ip,
                operation,
                state: Mutex::new(RequestRecorderState::default()),
            })),
        }
    }

    pub(crate) fn set_request_metadata(
        &self,
        public_model: String,
        is_stream: bool,
        thinking_level: Option<String>,
    ) {
        let Some(inner) = &self.inner else {
            return;
        };
        let mut state = inner.state.lock().expect("request recorder state");
        state.public_model = Some(public_model);
        state.is_stream = is_stream;
        state.thinking_level = thinking_level;
    }

    pub(crate) fn begin_attempt(
        &self,
        attempt_no: u32,
        candidate: &RouteCandidate,
        bound: bool,
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
        let mut state = inner.state.lock().expect("request recorder state");
        state.final_target = Some(target);
        state.quota_cost_rate = None;
        drop(state);
        AttemptRecorder::new(
            self.clone(),
            inner.request_id,
            attempt_no,
            candidate,
            bound,
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

    pub(super) fn push_attempt(&self, attempt: RequestAttempt) {
        let Some(inner) = &self.inner else {
            return;
        };
        let mut state = inner.state.lock().expect("request recorder state");
        if !state.finished {
            state.attempts.push(attempt);
        }
    }

    pub(super) fn observe_quota_cost_rate(&self, rate: Option<RequestQuotaCostRate>) {
        let Some(inner) = &self.inner else {
            return;
        };
        let mut state = inner.state.lock().expect("request recorder state");
        if !state.finished {
            state.quota_cost_rate = rate;
        }
    }

    pub(crate) fn annotate_attempt(
        &self,
        attempt_no: u32,
        failure_scope: Option<RequestAttemptFailureScope>,
        retry_decision: RequestAttemptRetryDecision,
    ) {
        let Some(inner) = &self.inner else {
            return;
        };
        let mut state = inner.state.lock().expect("request recorder state");
        if state.finished {
            return;
        }
        if let Some(attempt) = state
            .attempts
            .iter_mut()
            .find(|attempt| attempt.attempt_no == attempt_no)
        {
            attempt.failure_scope = failure_scope;
            attempt.retry_decision = Some(retry_decision);
        }
    }
}

pub(crate) const fn public_error_class(code: PublicErrorCode) -> ErrorClass {
    match code {
        PublicErrorCode::Unauthorized => ErrorClass::Authentication,
        PublicErrorCode::InvalidRequest
        | PublicErrorCode::PayloadTooLarge
        | PublicErrorCode::PublicApiNotFound
        | PublicErrorCode::MethodNotAllowed
        | PublicErrorCode::ModelNotFound => ErrorClass::InvalidRequest,
        PublicErrorCode::LocalRateLimit => ErrorClass::RateLimited,
        PublicErrorCode::InternalError => ErrorClass::Internal,
        PublicErrorCode::GatewayTimeout => ErrorClass::Network,
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

pub(super) fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

pub(super) fn bound_optional_error_message(message: impl AsRef<str>) -> Option<String> {
    let bounded = bound_error_message(message);
    (!bounded.is_empty()).then_some(bounded)
}
