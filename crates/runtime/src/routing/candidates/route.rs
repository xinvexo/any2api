use std::collections::BTreeMap;
use std::sync::Arc;

use any2api_domain::{
    ModelRoute, ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderEndpointId,
    ProviderKind, ProxyProfileId, RouteTargetId, RoutingCredentialId, TransportMode,
};
use any2api_protocol::api::{ProtocolRegistry, RequestExecutionProfile};
use any2api_provider::api::ProviderRegistry;

use super::{CandidateIdentity, EgressPathIdentity};
use super::{OAuthRoute, oauth};
use crate::credential::CredentialFilterKind;
use crate::health::{AttemptHealth, CandidatePathKey, EgressPathKey, HealthAcquireError};
use crate::health::{EndpointHealthRuntime, ProxyHealthRuntime, ReliabilityPolicy};
use crate::{
    configuration::PublishedSnapshot,
    credential::{CredentialRuntimeBinding, RoutingPermit},
    routing::{RouteAdmission, RoutingCredential},
};

#[derive(Clone, Debug)]
pub(crate) struct RouteCandidate {
    pub(crate) target_id: RouteTargetId,
    pub(crate) operation: ProtocolOperation,
    pub(crate) endpoint_id: ProviderEndpointId,
    pub(crate) endpoint_config_version: u64,
    pub(crate) credential_id: RoutingCredentialId,
    pub(crate) routing_generation: u64,
    pub(crate) provider_kind: ProviderKind,
    pub(crate) base_url: ProviderBaseUrl,
    pub(crate) upstream_model: String,
    pub(crate) upstream_protocol_dialect: ProtocolDialect,
    pub(crate) proxy_id: ProxyProfileId,
    pub(crate) proxy_config_version: u64,
    pub(crate) endpoint_health: Option<Arc<EndpointHealthRuntime>>,
    pub(crate) proxy_health: Option<Arc<ProxyHealthRuntime>>,
    pub(crate) egress_path_health: Arc<EndpointHealthRuntime>,
    pub(crate) candidate_path_health: Arc<EndpointHealthRuntime>,
    pub(crate) binding: CredentialRuntimeBinding,
    pub(crate) route_admission: Arc<RouteAdmission>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CandidateRequirements {
    operation: ProtocolOperation,
    execution_profile: RequestExecutionProfile,
    transport_mode: TransportMode,
}

impl CandidateRequirements {
    pub(crate) const fn new(
        operation: ProtocolOperation,
        execution_profile: RequestExecutionProfile,
        transport_mode: TransportMode,
    ) -> Self {
        Self {
            operation,
            execution_profile,
            transport_mode,
        }
    }

    pub(super) const fn operation(self) -> ProtocolOperation {
        self.operation
    }

    pub(super) const fn execution_profile(self) -> RequestExecutionProfile {
        self.execution_profile
    }

    pub(super) const fn transport_mode(self) -> TransportMode {
        self.transport_mode
    }
}

impl std::hash::Hash for CandidateRequirements {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.operation.hash(state);
        self.transport_mode.hash(state);
        // RequestExecutionProfile does not derive Hash; encode it exhaustively
        // so a new variant fails to compile instead of colliding.
        let execution_profile: u8 = match self.execution_profile {
            RequestExecutionProfile::Standard => 0,
            RequestExecutionProfile::RemoteCompaction => 1,
        };
        execution_profile.hash(state);
    }
}

impl RouteCandidate {
    pub(crate) fn admission_active(&self) -> bool {
        self.route_admission.is_active()
    }

    pub(crate) const fn identity(&self) -> CandidateIdentity {
        CandidateIdentity {
            target_id: self.target_id,
            operation: self.operation,
            credential_id: self.credential_id,
            routing_generation: self.routing_generation,
            endpoint_id: self.endpoint_id,
            endpoint_config_version: self.endpoint_config_version,
            proxy_id: self.proxy_id,
            proxy_config_version: self.proxy_config_version,
        }
    }

    pub(crate) const fn egress_path_identity(&self) -> EgressPathIdentity {
        EgressPathIdentity {
            endpoint_id: self.endpoint_id,
            endpoint_config_version: self.endpoint_config_version,
            proxy_id: self.proxy_id,
            proxy_config_version: self.proxy_config_version,
        }
    }

    pub(super) const fn credential_generation_identity(
        &self,
    ) -> super::identity::CredentialGenerationIdentity {
        super::identity::CredentialGenerationIdentity {
            credential_id: self.credential_id,
            routing_generation: self.routing_generation,
        }
    }

    pub(crate) fn health_availability(
        &self,
        policy: &ReliabilityPolicy,
    ) -> Result<(), CandidateHealthError> {
        self.binding
            .generation()
            .health()
            .availability(&self.upstream_model)
            .map_err(|error| {
                CandidateHealthError::new(CredentialFilterKind::CredentialHealth, error)
            })?;
        if let Some(endpoint) = &self.endpoint_health {
            endpoint.availability(policy).map_err(|error| {
                CandidateHealthError::new(CredentialFilterKind::EndpointHealth, error)
            })?;
        }
        if let Some(proxy) = &self.proxy_health {
            proxy.availability(policy).map_err(|error| {
                CandidateHealthError::new(CredentialFilterKind::ProxyHealth, error)
            })?;
        }
        self.egress_path_health
            .availability(policy)
            .map_err(|error| {
                CandidateHealthError::new(CredentialFilterKind::EgressPathHealth, error)
            })?;
        self.candidate_path_health
            .availability(policy)
            .map_err(|error| {
                CandidateHealthError::new(CredentialFilterKind::CandidateHealth, error)
            })?;
        Ok(())
    }

    pub(crate) fn acquire_health(
        &self,
        policy: ReliabilityPolicy,
    ) -> Result<AttemptHealth, CandidateHealthError> {
        self.binding
            .generation()
            .health()
            .availability(&self.upstream_model)
            .map_err(|error| {
                CandidateHealthError::new(CredentialFilterKind::CredentialHealth, error)
            })?;
        let endpoint = match &self.endpoint_health {
            Some(endpoint) => Some(endpoint.try_acquire(&policy).map_err(|error| {
                CandidateHealthError::new(CredentialFilterKind::EndpointHealth, error)
            })?),
            None => None,
        };
        let proxy = match &self.proxy_health {
            Some(proxy) => match proxy.try_acquire(&policy) {
                Ok(proxy) => Some(proxy),
                Err(error) => {
                    drop(endpoint);
                    return Err(CandidateHealthError::new(
                        CredentialFilterKind::ProxyHealth,
                        error,
                    ));
                }
            },
            None => None,
        };
        let egress_path = match self.egress_path_health.try_acquire(&policy) {
            Ok(permit) => permit,
            Err(error) => {
                drop(endpoint);
                drop(proxy);
                return Err(CandidateHealthError::new(
                    CredentialFilterKind::EgressPathHealth,
                    error,
                ));
            }
        };
        let candidate_path = match self.candidate_path_health.try_acquire(&policy) {
            Ok(permit) => permit,
            Err(error) => {
                drop(endpoint);
                drop(proxy);
                drop(egress_path);
                return Err(CandidateHealthError::new(
                    CredentialFilterKind::CandidateHealth,
                    error,
                ));
            }
        };
        Ok(AttemptHealth::new_with_paths(
            Arc::clone(self.binding.generation()),
            self.upstream_model.clone(),
            endpoint,
            proxy,
            Some(egress_path),
            Some(candidate_path),
            policy,
        ))
    }

    pub(crate) fn acquire_health_with_rpm_reservation(
        &self,
        policy: ReliabilityPolicy,
        permit: RoutingPermit,
    ) -> Result<(RoutingPermit, AttemptHealth), CandidateHealthError> {
        match self.acquire_health(policy) {
            Ok(health) => Ok((permit, health)),
            Err(error) => {
                permit.rollback_before_attempt();
                Err(error)
            }
        }
    }

    pub(crate) fn record_filter(&self, kind: CredentialFilterKind) {
        self.binding.record_filter(kind);
    }

    pub(crate) fn record_selection(&self) {
        self.binding.record_selection();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CandidateHealthError {
    kind: CredentialFilterKind,
    source: HealthAcquireError,
}

impl CandidateHealthError {
    const fn new(kind: CredentialFilterKind, source: HealthAcquireError) -> Self {
        Self { kind, source }
    }

    pub(crate) const fn kind(self) -> CredentialFilterKind {
        self.kind
    }

    pub(crate) const fn source(self) -> HealthAcquireError {
        self.source
    }
}

pub(crate) fn build_route_candidates(
    snapshot: &PublishedSnapshot,
    route: &ModelRoute,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
    requirements: CandidateRequirements,
) -> BTreeMap<u16, Vec<RouteCandidate>> {
    let mut tiers = BTreeMap::new();
    for target in route.targets().iter().filter(|target| target.enabled()) {
        let Some(endpoint) = snapshot
            .provider_endpoints()
            .get(target.provider_endpoint_id())
        else {
            continue;
        };
        if !endpoint.enabled()
            || endpoint.protocol_dialect() != route.ingress_protocol()
            || endpoint.effective_upstream_protocol_dialect() != target.upstream_protocol_dialect()
            || requirements.execution_profile().requires_same_dialect()
                && target.upstream_protocol_dialect() != route.ingress_protocol()
            || !protocols.supports_operation(
                route.ingress_protocol(),
                target.upstream_protocol_dialect(),
                requirements.operation(),
            )
        {
            continue;
        }
        let Some(driver) = providers.get(endpoint.provider_kind()) else {
            continue;
        };
        // A direct API Key target must support this exact ingress operation.
        // A bridge maps it to another operation later; that final operation is
        // validated while building the prepared upstream request.
        if route.ingress_protocol() == target.upstream_protocol_dialect()
            && !driver.supports_api_key_operation(requirements.operation())
        {
            continue;
        }
        let capabilities = driver.capabilities();
        if !capabilities
            .protocols
            .contains(&target.upstream_protocol_dialect())
            || !capabilities
                .transport_modes
                .contains(&requirements.transport_mode())
        {
            continue;
        }

        for credential in snapshot
            .routing_credentials()
            .iter()
            .filter(|credential| !credential.is_oauth())
            .filter(|credential| credential.endpoint_id() == endpoint.id())
            .filter(|credential| credential.routable())
            .filter(|credential| credential.supports_model(target.upstream_model()))
        {
            let Some(route_admission) = credential.route_admission().cloned() else {
                continue;
            };
            let Some(proxy) = snapshot.proxies().get(credential.proxy_id()) else {
                continue;
            };
            if !proxy.enabled() {
                continue;
            }
            let endpoint_health = snapshot.endpoint_health(endpoint.id()).cloned();
            let proxy_health = snapshot.proxy_health(proxy.id()).cloned();
            let (egress_path_health, candidate_path_health) =
                path_health(snapshot, target.id(), requirements.operation(), credential);

            tiers
                .entry(target.fallback_tier().get())
                .or_insert_with(Vec::new)
                .push(RouteCandidate {
                    target_id: target.id(),
                    operation: requirements.operation(),
                    endpoint_id: endpoint.id(),
                    endpoint_config_version: credential.endpoint_config_version(),
                    credential_id: credential.id(),
                    routing_generation: credential.binding().generation().routing_generation(),
                    provider_kind: credential.provider_kind(),
                    base_url: credential.base_url().clone(),
                    upstream_model: target.upstream_model().as_str().to_owned(),
                    upstream_protocol_dialect: target.upstream_protocol_dialect(),
                    proxy_id: proxy.id(),
                    proxy_config_version: credential.proxy_config_version(),
                    endpoint_health,
                    proxy_health,
                    egress_path_health,
                    candidate_path_health,
                    binding: credential.binding().clone(),
                    route_admission,
                });
        }
    }
    oauth::add_oauth_candidates(
        &mut tiers,
        snapshot,
        OAuthRoute::new(route.id(), route.ingress_protocol(), route.public_model()),
        protocols,
        providers,
        requirements,
    );
    tiers
}

pub(super) fn path_health(
    snapshot: &PublishedSnapshot,
    target_id: RouteTargetId,
    operation: ProtocolOperation,
    credential: &RoutingCredential,
) -> (Arc<EndpointHealthRuntime>, Arc<EndpointHealthRuntime>) {
    let endpoint_id = credential.endpoint_id();
    let endpoint_config_version = credential.endpoint_config_version();
    let proxy_id = credential.proxy_id();
    let proxy_config_version = credential.proxy_config_version();
    let egress = snapshot.egress_path_health(EgressPathKey {
        endpoint_id,
        endpoint_config_version,
        proxy_id,
        proxy_config_version,
    });
    let candidate = snapshot.candidate_path_health(CandidatePathKey {
        target_id,
        operation,
        credential_id: credential.id(),
        routing_generation: credential.binding().generation().routing_generation(),
        endpoint_id,
        endpoint_config_version,
        proxy_id,
        proxy_config_version,
    });
    (egress, candidate)
}
