use std::sync::Arc;

use any2api_domain::{
    ProtocolOperation, ProxyProfile, PublicError, PublicErrorCode, UpstreamErrorClassification,
};
use any2api_protocol::api::{DecodedRequest, ProtocolAdapter};
use any2api_provider::api::{ProviderDriver, ProviderRegistry, UpstreamResponseMeta};
use any2api_transport::api::{
    EndpointNetworkPolicy, TransportError, TransportFailureScope, TransportManager,
    TransportRequest, TransportResponse,
};

use super::super::{
    RequestPermit, SelectedCandidate,
    affinity::AffinitySelection,
    response::{MAX_CLASSIFIED_ERROR_BYTES, internal_error, invalid_request, public_error},
};
use super::failure::AttemptFailure;
use crate::{
    affinity::{AffinityTarget, HardAffinityCommitter, SoftBindingLease},
    health::AttemptHealth,
    published_snapshot::PublishedSnapshot,
    route_candidates::RouteCandidate,
};

pub(super) struct AttemptInput<'a> {
    pub(super) prepared: PreparedAttempt<'a>,
    pub(super) candidate: RouteCandidate,
    pub(super) target: AffinityTarget,
    pub(super) soft_lease: Option<SoftBindingLease>,
    pub(super) fixed: bool,
}

pub(super) fn prepare_input<'a>(
    snapshot: &'a PublishedSnapshot,
    adapter: &dyn ProtocolAdapter,
    decoded: DecodedRequest,
    affinity: AffinitySelection,
    providers: &'a ProviderRegistry,
) -> Result<AttemptInput<'a>, AttemptFailure> {
    let AffinitySelection {
        selected,
        target,
        soft_lease,
        fixed,
    } = affinity;
    let candidate = selected.candidate.clone();
    let prepared = prepare_attempt(snapshot, adapter, decoded, selected, providers)
        .map_err(AttemptFailure::Public)?;
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
    proxy: &'a ProxyProfile,
    pub(super) operation: ProtocolOperation,
    request: Option<TransportRequest>,
    permit: Option<RequestPermit>,
    health: Option<AttemptHealth>,
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
            self.operation,
            &UpstreamResponseMeta {
                status,
                headers: headers.clone(),
            },
            &body[..body.len().min(MAX_CLASSIFIED_ERROR_BYTES)],
        )
    }

    pub(super) fn success(&mut self) {
        if let Some(health) = self.health.take() {
            health.success();
        }
        self.permit.take();
    }

    pub(super) fn fail_after_upstream_success(&mut self, error: PublicError) -> AttemptFailure {
        self.success();
        AttemptFailure::Public(error)
    }

    pub(super) fn upstream_failure(&mut self, classification: UpstreamErrorClassification) {
        if let Some(health) = self.health.take() {
            health.upstream_failure(classification);
        }
        self.permit.take();
    }

    pub(super) fn transport_failure(&mut self, error: &TransportError) {
        if let Some(health) = self.health.take() {
            health.transport_failure(error.failure_scope);
        }
        self.permit.take();
    }

    pub(super) fn invalid_response(&mut self) {
        if let Some(health) = self.health.take() {
            health.transport_failure(TransportFailureScope::Endpoint);
        }
        self.permit.take();
    }

    pub(super) fn take_guards(&mut self) -> (RequestPermit, Option<AttemptHealth>) {
        (
            self.permit.take().expect("prepared permit is present"),
            self.health.take(),
        )
    }
}

fn prepare_attempt<'a>(
    snapshot: &'a PublishedSnapshot,
    adapter: &dyn ProtocolAdapter,
    decoded: DecodedRequest,
    selected: SelectedCandidate,
    providers: &'a ProviderRegistry,
) -> Result<PreparedAttempt<'a>, PublicError> {
    let SelectedCandidate {
        candidate,
        permit,
        health,
    } = selected;
    let endpoint = snapshot
        .provider_endpoints()
        .get(candidate.endpoint_id)
        .ok_or_else(internal_error)?;
    let driver = providers
        .get(endpoint.provider_kind())
        .ok_or_else(internal_error)?
        .as_ref();
    let proxy = snapshot
        .resolved_proxy_for_credential(candidate.credential_id)
        .filter(|proxy| proxy.enabled())
        .ok_or_else(|| {
            public_error(
                PublicErrorCode::NoAvailableCredential,
                "configured proxy is unavailable",
            )
        })?;
    let endpoint_plan = driver
        .endpoint_plan(endpoint.base_url(), decoded.operation)
        .map_err(|_| internal_error())?;
    let operation = decoded.operation;
    let mut encoded = adapter
        .encode_upstream_request(
            decoded.operation,
            decoded.headers,
            decoded.payload,
            &candidate.upstream_model,
        )
        .map_err(|_| invalid_request("request body could not be encoded"))?;
    encoded.uri = endpoint_plan
        .url
        .as_str()
        .parse()
        .map_err(|_| internal_error())?;
    let credential_headers = permit
        .provider_credential_headers(driver)
        .map_err(|_| internal_error())?;
    encoded.headers.extend(credential_headers.headers);
    Ok(PreparedAttempt {
        driver,
        proxy,
        operation,
        request: Some(TransportRequest {
            method: encoded.method,
            uri: encoded.uri,
            headers: encoded.headers,
            body: encoded.body,
            network_policy: EndpointNetworkPolicy::new(endpoint.allow_private_network()),
        }),
        permit: Some(permit),
        health: Some(health),
    })
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
mod tests {
    use std::sync::Arc;

    use any2api_domain::{
        CredentialId, CredentialKind, CredentialSecretFingerprint, MaxConcurrency,
        ProtocolOperation, ProviderCredential, ProviderCredentialDraft, ProviderEndpointId,
        ProxyProfile, ProxyProfileId, PublicErrorCode,
    };
    use any2api_provider::{CodexDriver, api::ProviderDriver};

    use super::PreparedAttempt;
    use crate::{
        credential_auth::CredentialAuthMaterial,
        credential_runtime::CredentialRuntimeHandle,
        health::{AttemptHealth, EndpointHealthRuntime, ReliabilityPolicy},
        public_request::{RequestPermit, response::public_error},
        scheduler_epoch::SchedulerEpoch,
    };

    #[tokio::test(start_paused = true)]
    async fn postprocess_failure_closes_half_open_health_before_releasing_capacity() {
        let epoch = SchedulerEpoch::new();
        let policy = ReliabilityPolicy::from_settings(
            any2api_domain::SettingsConfiguration::defaults().reliability(),
        );
        let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
        let endpoint_permits = (0..policy.endpoint_failure_threshold)
            .map(|_| endpoint.try_acquire(&policy).expect("closed endpoint"))
            .collect::<Vec<_>>();
        for permit in endpoint_permits {
            permit.failure(&policy);
        }
        tokio::time::advance(policy.endpoint_open_duration).await;

        let credential = ProviderCredential::create(
            CredentialId::new(),
            ProviderEndpointId::new(),
            ProviderCredentialDraft::new(
                "postprocess",
                CredentialKind::ApiKey,
                ProxyProfileId::DIRECT,
                MaxConcurrency::new(1).expect("max concurrency"),
                true,
            )
            .expect("credential draft"),
            CredentialSecretFingerprint::new([7; 32], None).expect("fingerprint"),
        );
        let binding = CredentialRuntimeHandle::new(
            &credential,
            CredentialAuthMaterial::for_test(&credential, "sk-postprocess-test".into()),
            epoch,
        )
        .current_binding();
        let permit = binding.try_acquire().expect("credential permit");
        let health = AttemptHealth::new(
            Arc::clone(binding.generation()),
            "upstream-model".into(),
            Some(endpoint.try_acquire(&policy).expect("half-open probe")),
            None,
            policy,
        );
        let driver = CodexDriver::new();
        let proxy = ProxyProfile::direct();
        let mut prepared = PreparedAttempt {
            driver: &driver as &dyn ProviderDriver,
            proxy: &proxy,
            operation: ProtocolOperation::Responses,
            request: None,
            permit: Some(RequestPermit::Generation(permit)),
            health: Some(health),
        };

        let failure = prepared.fail_after_upstream_success(public_error(
            PublicErrorCode::InternalError,
            "test postprocess failure",
        ));

        assert!(matches!(failure, super::AttemptFailure::Public(_)));
        assert_eq!(binding.capacity().in_flight(), 0);
        let first = endpoint
            .try_acquire(&policy)
            .expect("closed endpoint first permit");
        let second = endpoint
            .try_acquire(&policy)
            .expect("closed endpoint second permit");
        drop(first);
        drop(second);
    }
}
