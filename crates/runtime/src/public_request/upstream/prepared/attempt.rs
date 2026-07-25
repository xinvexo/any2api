use std::sync::Arc;

use any2api_domain::{
    ErrorClass, ProtocolOperation, PublicError, TokenUsage, UpstreamErrorClassification,
};
use any2api_protocol::{
    ProtocolError,
    api::{
        DecodedRequest, DecodedUpstreamResponse, EgressResponse, ProtocolExchange,
        ProtocolRegistry, UpstreamResponse,
    },
};
use any2api_provider::api::{ProviderDriver, ProviderRegistry, UpstreamResponseMeta};
use any2api_transport::api::{
    TransportError, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};

use super::super::super::{
    RequestPermit,
    affinity::AffinitySelection,
    response::{MAX_CLASSIFIED_ERROR_BYTES, transport_error_diagnostic},
};
use super::super::failure::AttemptFailure;
use crate::{
    affinity::{AffinityTarget, HardAffinityCommitter, SoftBindingLease},
    configuration::PublishedSnapshot,
    health::AttemptHealth,
    request_telemetry::{AttemptRecorder, public_error_class},
    routing::RouteCandidate,
};

use super::build::prepare_attempt;

pub(in crate::public_request::upstream) struct AttemptInput<'a> {
    pub(in crate::public_request::upstream) prepared: PreparedAttempt<'a>,
    pub(in crate::public_request::upstream) candidate: RouteCandidate,
    pub(in crate::public_request::upstream) target: AffinityTarget,
    pub(in crate::public_request::upstream) soft_lease: Option<SoftBindingLease>,
    pub(in crate::public_request::upstream) fixed: bool,
}

pub(in crate::public_request::upstream) fn prepare_input<'a>(
    snapshot: &'a PublishedSnapshot,
    protocols: &ProtocolRegistry,
    decoded: DecodedRequest,
    affinity: AffinitySelection,
    providers: &'a ProviderRegistry,
    attempt_recorder: AttemptRecorder,
) -> Result<AttemptInput<'a>, AttemptFailure> {
    let AffinitySelection {
        selected,
        target,
        soft_lease,
        fixed,
    } = affinity;
    let candidate = selected.candidate.clone();
    let prepared = prepare_attempt(
        snapshot,
        protocols,
        decoded,
        selected,
        providers,
        attempt_recorder,
    )?;
    Ok(AttemptInput {
        prepared,
        candidate,
        target,
        soft_lease,
        fixed,
    })
}

pub(in crate::public_request::upstream) struct PreparedAttempt<'a> {
    pub(super) driver: &'a dyn ProviderDriver,
    pub(super) proxy: TransportProxy<'a>,
    pub(in crate::public_request::upstream) ingress_operation: ProtocolOperation,
    pub(super) upstream_operation: ProtocolOperation,
    pub(super) exchange: Option<ProtocolExchange>,
    pub(super) request: Option<TransportRequest>,
    pub(super) permit: Option<RequestPermit>,
    pub(super) health: Option<AttemptHealth>,
    pub(super) attempt_recorder: Option<AttemptRecorder>,
}

impl PreparedAttempt<'_> {
    pub(in crate::public_request::upstream) async fn send(
        &mut self,
        transport: &dyn TransportManager,
    ) -> Result<TransportResponse, TransportError> {
        let request = self.request.take().expect("prepared request is present");
        transport.execute(self.proxy, request).await
    }

    pub(in crate::public_request::upstream) fn classify(
        &self,
        status: http::StatusCode,
        headers: &http::HeaderMap,
        body: &[u8],
    ) -> UpstreamErrorClassification {
        self.driver.classify_error(
            self.upstream_operation,
            &UpstreamResponseMeta {
                status,
                headers: headers.clone(),
            },
            &body[..body.len().min(MAX_CLASSIFIED_ERROR_BYTES)],
        )
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

    pub(in crate::public_request::upstream) fn observe_token_usage(&self, usage: TokenUsage) {
        if let Some(recorder) = &self.attempt_recorder {
            recorder.observe_token_usage(usage);
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

    pub(in crate::public_request::upstream) fn hard_affinity_id_from_response(
        &self,
        response: &DecodedUpstreamResponse,
    ) -> Result<Option<String>, ProtocolError> {
        self.exchange
            .as_ref()
            .expect("prepared protocol exchange is present")
            .hard_affinity_id_from_response(self.ingress_operation, response)
    }

    pub(in crate::public_request::upstream) fn encode_egress_response(
        &self,
        response: DecodedUpstreamResponse,
    ) -> Result<EgressResponse, ProtocolError> {
        self.exchange
            .as_ref()
            .expect("prepared protocol exchange is present")
            .encode_egress_response(response)
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
                public_error_class(error.code),
                &error.message,
            );
        }
        self.permit.take();
        AttemptFailure::Public(error)
    }

    pub(in crate::public_request::upstream) fn upstream_failure(
        &mut self,
        status_code: u16,
        classification: UpstreamErrorClassification,
    ) {
        if let Some(health) = self.health.take() {
            health.upstream_failure(classification);
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.upstream_error(
                status_code,
                classification.retry_safety(),
                classification.kind().error_class(),
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
                TransportFailureScope::Endpoint | TransportFailureScope::Unattributed => {
                    ErrorClass::Network
                }
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

    pub(in crate::public_request::upstream) fn take_guards(
        &mut self,
    ) -> (
        ProtocolExchange,
        RequestPermit,
        Option<AttemptHealth>,
        AttemptRecorder,
    ) {
        (
            self.exchange
                .take()
                .expect("prepared protocol exchange is present"),
            self.permit.take().expect("prepared permit is present"),
            self.health.take(),
            self.attempt_recorder
                .take()
                .expect("prepared attempt recorder is present"),
        )
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

pub(in crate::public_request::upstream) fn hard_committer(
    snapshot: &PublishedSnapshot,
    operation: ProtocolOperation,
    target: AffinityTarget,
) -> HardAffinityCommitter {
    HardAffinityCommitter::new(
        operation,
        Arc::clone(snapshot.affinity_registry()),
        target,
        snapshot.affinity_policy().hard_ttl(),
    )
}
