use std::sync::Arc;

use any2api_domain::{
    ErrorClass, OAuthAccountId, ProtocolOperation, PublicError, RequestSpeedTier, TokenUsage,
    UpstreamError,
};
use any2api_protocol::api::{
    BridgeContinuationState, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
    ProtocolError, ProtocolExchange, ProtocolUpstreamFailureEvidence, UpstreamResponse,
};
use any2api_provider::api::{ProviderDriver, UpstreamResponseMeta};
use any2api_transport::api::{
    TransportError, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};

use super::super::super::{
    RequestPermit,
    affinity::AffinitySelection,
    response::{MAX_UPSTREAM_ERROR_BODY_BYTES, transport_error_diagnostic},
};
use super::super::UpstreamServices;
use super::super::failure::AttemptFailure;
use crate::{
    affinity::{AffinityTarget, BindingLease, ContinuationBindingCommitter},
    configuration::PublishedSnapshot,
    health::AttemptHealth,
    oauth::{OAuthQuotaActivity, OAuthQuotaActivityGuard},
    request_telemetry::{AttemptRecorder, public_error_class},
    routing::RouteCandidate,
};

use super::build::{AttemptHeaderPolicy, AttemptPreparation, prepare_attempt};

pub(in crate::public_request::upstream) struct AttemptInput<'a> {
    pub(in crate::public_request::upstream) prepared: PreparedAttempt<'a>,
    pub(in crate::public_request::upstream) candidate: RouteCandidate,
    pub(in crate::public_request::upstream) target: AffinityTarget,
    pub(in crate::public_request::upstream) binding_lease: Option<BindingLease>,
    pub(in crate::public_request::upstream) bound: bool,
}

pub(in crate::public_request::upstream) struct PreparedStreamGuards {
    pub(in crate::public_request::upstream) exchange: ProtocolExchange,
    pub(in crate::public_request::upstream) permit: RequestPermit,
    pub(in crate::public_request::upstream) health: Option<AttemptHealth>,
    pub(in crate::public_request::upstream) attempt_recorder: AttemptRecorder,
    pub(in crate::public_request::upstream) quota_activity: Option<OAuthQuotaActivityGuard>,
    pub(in crate::public_request::upstream) driver: Arc<dyn ProviderDriver>,
    pub(in crate::public_request::upstream) upstream_operation: ProtocolOperation,
}

pub(in crate::public_request::upstream) fn prepare_input<'a>(
    services: UpstreamServices<'a>,
    decoded: &DecodedRequest,
    affinity: AffinitySelection,
    attempt_recorder: AttemptRecorder,
    allow_credential_bound_headers: bool,
) -> Result<AttemptInput<'a>, AttemptFailure> {
    let AffinitySelection {
        selected,
        target,
        binding_lease,
        bound,
        continuation_state,
    } = affinity;
    let candidate = selected.candidate.clone();
    let mut prepared = prepare_attempt(AttemptPreparation {
        policy_snapshot: services.policy_snapshot,
        routing_snapshot: services.routing_snapshot,
        protocols: services.protocols,
        decoded,
        selected,
        continuation_state,
        providers: services.providers,
        attempt_recorder,
        header_policy: AttemptHeaderPolicy {
            allow_credential_bound: allow_credential_bound_headers,
            allow_turn_state: bound && allow_credential_bound_headers,
        },
    })?;
    prepared.quota_activity = services.oauth_quota_activity;
    prepared.oauth_account_id = candidate.credential_id.oauth_account_id();
    Ok(AttemptInput {
        prepared,
        candidate,
        target,
        binding_lease,
        bound,
    })
}

pub(in crate::public_request::upstream) struct PreparedAttempt<'a> {
    pub(super) driver: Arc<dyn ProviderDriver>,
    pub(super) proxy: TransportProxy<'a>,
    pub(in crate::public_request::upstream) ingress_operation: ProtocolOperation,
    pub(super) upstream_operation: ProtocolOperation,
    pub(super) exchange: Option<ProtocolExchange>,
    pub(super) request: Option<TransportRequest>,
    pub(super) permit: Option<RequestPermit>,
    pub(super) health: Option<AttemptHealth>,
    pub(super) attempt_recorder: Option<AttemptRecorder>,
    pub(super) quota_activity: Option<&'a OAuthQuotaActivity>,
    pub(super) oauth_account_id: Option<OAuthAccountId>,
    pub(super) quota_activity_guard: Option<OAuthQuotaActivityGuard>,
}

impl PreparedAttempt<'_> {
    pub(in crate::public_request::upstream) async fn send(
        &mut self,
        transport: &dyn TransportManager,
    ) -> Result<TransportResponse, TransportError> {
        let request = self.request.take().expect("prepared request is present");
        if let Some(diagnostics) = transport.request_diagnostics(self.proxy, &request)
            && let Some(recorder) = self.attempt_recorder.as_mut()
        {
            recorder.observe_transport(diagnostics);
        }
        if self.quota_activity_guard.is_none()
            && let Some(activity) = self.quota_activity
            && let Some(id) = self.oauth_account_id
        {
            self.quota_activity_guard = Some(activity.guard(id));
        }
        transport.execute(self.proxy, request).await
    }

    pub(in crate::public_request::upstream) fn classify(
        &self,
        status: http::StatusCode,
        headers: &http::HeaderMap,
        body: &[u8],
    ) -> UpstreamError {
        self.driver.classify_error(
            self.upstream_operation,
            &UpstreamResponseMeta {
                status,
                headers: headers.clone(),
            },
            &body[..body.len().min(MAX_UPSTREAM_ERROR_BODY_BYTES)],
        )
    }

    pub(in crate::public_request::upstream) fn classify_evidence(
        &self,
        status: http::StatusCode,
        headers: &http::HeaderMap,
        evidence: &ProtocolUpstreamFailureEvidence,
    ) -> UpstreamError {
        let classified = self.classify(status, headers, evidence.raw_json());
        match evidence.retry_safety_override() {
            Some(safety) => classified.with_retry_safety(safety),
            None => classified,
        }
    }

    pub(in crate::public_request::upstream) fn buffered_upstream_failure(
        &self,
        response: &UpstreamResponse,
    ) -> Option<ProtocolUpstreamFailureEvidence> {
        self.exchange
            .as_ref()
            .expect("prepared protocol exchange is present")
            .buffered_upstream_failure(response)
    }

    pub(in crate::public_request::upstream) fn response_headers(
        &self,
        upstream: &http::HeaderMap,
    ) -> http::HeaderMap {
        self.driver
            .response_headers(self.upstream_operation, upstream)
    }

    pub(in crate::public_request::upstream) fn success(&mut self, status_code: u16) {
        if let Some(health) = self.health.take() {
            health.success();
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.success(status_code);
        }
        self.permit.take();
    }

    pub(in crate::public_request::upstream) fn observe_token_usage(&mut self, usage: TokenUsage) {
        if let Some(recorder) = &self.attempt_recorder {
            recorder.observe_token_usage(usage);
        }
    }

    pub(in crate::public_request::upstream) fn observe_effective_speed_tier(
        &mut self,
        tier: Option<RequestSpeedTier>,
    ) {
        let tier = self
            .driver
            .response_speed_tier(self.upstream_operation, tier);
        if let Some(recorder) = &self.attempt_recorder {
            recorder.observe_effective_speed_tier(tier);
        }
    }

    pub(in crate::public_request::upstream) fn decode_upstream_response(
        &mut self,
        response: UpstreamResponse,
    ) -> Result<DecodedUpstreamResponse, ProtocolError> {
        self.exchange
            .as_mut()
            .expect("prepared protocol exchange is present")
            .decode_upstream_response(response)
    }

    pub(in crate::public_request::upstream) fn continuation_id_from_response(
        &self,
        response: &DecodedUpstreamResponse,
    ) -> Result<Option<String>, ProtocolError> {
        self.exchange
            .as_ref()
            .expect("prepared protocol exchange is present")
            .continuation_id_from_response(self.ingress_operation, response)
    }

    pub(in crate::public_request::upstream) fn bridge_continuation_state(
        &self,
    ) -> BridgeContinuationState {
        self.exchange
            .as_ref()
            .expect("prepared protocol exchange is present")
            .bridge_continuation_state()
    }

    pub(in crate::public_request::upstream) fn encode_egress_response(
        &self,
        response: DecodedUpstreamResponse,
        public_model: &str,
    ) -> Result<EgressResponse, ProtocolError> {
        self.exchange
            .as_ref()
            .expect("prepared protocol exchange is present")
            .encode_egress_response(response, public_model)
    }

    pub(in crate::public_request::upstream) fn fail_after_upstream_success(
        &mut self,
        status_code: u16,
        error: PublicError,
    ) -> AttemptFailure {
        if let Some(health) = self.health.take() {
            health.success();
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.local_error(
                Some(status_code),
                public_error_class(error.code()),
                error.telemetry_message(),
            );
        }
        self.permit.take();
        AttemptFailure::Public(error)
    }

    pub(in crate::public_request::upstream) fn upstream_failure(
        &mut self,
        status_code: u16,
        error: &UpstreamError,
    ) {
        let classification = error.classification();
        if let Some(health) = self.health.take() {
            health.upstream_failure(classification);
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.upstream_error(
                status_code,
                classification.retry_safety(),
                classification.kind().error_class(),
                error.official_message(),
            );
        }
        self.permit.take();
    }

    pub(in crate::public_request::upstream) fn transport_failure(
        &mut self,
        error: &TransportError,
    ) {
        if let Some(health) = self.health.take() {
            health.transport_failure(error.failure_scope);
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            let error_class = match error.failure_scope {
                TransportFailureScope::Proxy => ErrorClass::Proxy,
                TransportFailureScope::Endpoint
                | TransportFailureScope::EgressPath
                | TransportFailureScope::Unattributed => ErrorClass::Network,
            };
            recorder.transport_error(
                error.retry_safety,
                error_class,
                transport_error_diagnostic(error),
            );
        }
        self.permit.take();
    }

    pub(in crate::public_request::upstream) fn invalid_response(
        &mut self,
        status_code: Option<u16>,
        message: impl AsRef<str>,
    ) {
        if let Some(health) = self.health.take() {
            health.transport_failure(TransportFailureScope::Endpoint);
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.invalid_response(status_code, message);
        }
        self.permit.take();
    }

    pub(in crate::public_request::upstream) fn take_guards(&mut self) -> PreparedStreamGuards {
        PreparedStreamGuards {
            exchange: self
                .exchange
                .take()
                .expect("prepared protocol exchange is present"),
            permit: self.permit.take().expect("prepared permit is present"),
            health: self.health.take(),
            attempt_recorder: self
                .attempt_recorder
                .take()
                .expect("prepared attempt recorder is present"),
            quota_activity: self.quota_activity_guard.take(),
            driver: Arc::clone(&self.driver),
            upstream_operation: self.upstream_operation,
        }
    }
}

impl Drop for PreparedAttempt<'_> {
    fn drop(&mut self) {
        self.health.take();
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.cancelled(None);
        }
        self.permit.take();
    }
}

pub(in crate::public_request::upstream) fn continuation_committer(
    policy_snapshot: &PublishedSnapshot,
    operation: ProtocolOperation,
    target: AffinityTarget,
) -> ContinuationBindingCommitter {
    ContinuationBindingCommitter::new(
        operation,
        Arc::clone(policy_snapshot.affinity_registry()),
        target,
        policy_snapshot.affinity_policy().ttl(),
    )
}
