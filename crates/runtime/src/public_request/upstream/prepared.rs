use std::sync::Arc;

use any2api_domain::{
    ErrorClass, ProtocolOperation, PublicError, TokenUsage, UpstreamErrorClassification,
    extract_upstream_error_message,
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

use super::super::{
    RequestPermit, affinity::AffinitySelection, response::MAX_CLASSIFIED_ERROR_BYTES,
};
use super::failure::AttemptFailure;
use crate::{
    affinity::{AffinityTarget, HardAffinityCommitter, SoftBindingLease},
    health::AttemptHealth,
    published_snapshot::PublishedSnapshot,
    request_telemetry::{AttemptRecorder, public_error_class},
    route_candidates::RouteCandidate,
};

mod build;

use build::prepare_attempt;

pub(super) struct AttemptInput<'a> {
    pub(super) prepared: PreparedAttempt<'a>,
    pub(super) candidate: RouteCandidate,
    pub(super) target: AffinityTarget,
    pub(super) soft_lease: Option<SoftBindingLease>,
    pub(super) fixed: bool,
}

pub(super) fn prepare_input<'a>(
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

pub(super) struct PreparedAttempt<'a> {
    driver: &'a dyn ProviderDriver,
    proxy: TransportProxy<'a>,
    pub(super) ingress_operation: ProtocolOperation,
    upstream_operation: ProtocolOperation,
    exchange: Option<ProtocolExchange>,
    request: Option<TransportRequest>,
    permit: Option<RequestPermit>,
    health: Option<AttemptHealth>,
    attempt_recorder: Option<AttemptRecorder>,
}

impl PreparedAttempt<'_> {
    pub(super) async fn send(
        &mut self,
        transport: &dyn TransportManager,
    ) -> Result<TransportResponse, TransportError> {
        let request = self.request.take().expect("prepared request is present");
        transport.execute(self.proxy, request).await
    }

    pub(super) fn classify(
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

    pub(super) fn success(&mut self, status_code: u16) {
        if let Some(health) = self.health.take() {
            health.success();
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.success(status_code);
        }
        self.permit.take();
    }

    pub(super) fn observe_token_usage(&self, usage: TokenUsage) {
        if let Some(recorder) = &self.attempt_recorder {
            recorder.observe_token_usage(usage);
        }
    }

    pub(super) fn decode_upstream_response(
        &mut self,
        response: UpstreamResponse,
    ) -> Result<DecodedUpstreamResponse, ProtocolError> {
        self.exchange
            .as_mut()
            .expect("prepared protocol exchange is present")
            .decode_upstream_response(response)
    }

    pub(super) fn hard_affinity_id_from_response(
        &self,
        response: &DecodedUpstreamResponse,
    ) -> Result<Option<String>, ProtocolError> {
        self.exchange
            .as_ref()
            .expect("prepared protocol exchange is present")
            .hard_affinity_id_from_response(self.ingress_operation, response)
    }

    pub(super) fn encode_egress_response(
        &self,
        response: DecodedUpstreamResponse,
    ) -> Result<EgressResponse, ProtocolError> {
        self.exchange
            .as_ref()
            .expect("prepared protocol exchange is present")
            .encode_egress_response(response)
    }

    pub(super) fn fail_after_upstream_success(
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

    pub(super) fn upstream_failure(
        &mut self,
        status_code: u16,
        classification: UpstreamErrorClassification,
        body: &[u8],
    ) {
        if let Some(health) = self.health.take() {
            health.upstream_failure(classification);
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.upstream_error(
                status_code,
                classification.retry_safety(),
                classification.kind().error_class(),
                extract_upstream_error_message(body),
            );
        }
        self.permit.take();
    }

    pub(super) fn transport_failure(&mut self, error: &TransportError) {
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
            recorder.transport_error(error.retry_safety, error_class, &error.message);
        }
        self.permit.take();
    }

    pub(super) fn invalid_response(&mut self, status_code: Option<u16>, message: impl AsRef<str>) {
        if let Some(health) = self.health.take() {
            health.transport_failure(TransportFailureScope::Endpoint);
        }
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.invalid_response(status_code, message);
        }
        self.permit.take();
    }

    pub(super) fn take_guards(
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

pub(super) fn hard_committer(
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

#[cfg(test)]
mod tests;
