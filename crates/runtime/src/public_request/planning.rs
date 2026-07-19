use std::collections::BTreeMap;

use any2api_domain::{
    FallbackTier, ProtocolOperation, PublicError, PublicErrorCode, PublicModelName, TransportMode,
};
use any2api_protocol::api::{DecodedRequest, IngressRequest, ProtocolAdapter};
use any2api_provider::api::ProviderRegistry;
use http::{Method, Uri};

use super::{
    PublicRequest, RequestPermit, SelectedCandidate,
    response::{internal_error, invalid_request, public_error},
};
use crate::{
    auxiliary_scheduler::AuxiliarySelectAndAcquireResult,
    published_snapshot::PublishedSnapshot,
    route_candidates::{RouteCandidate, build_route_candidates},
    scheduler::{IndexedSelectAndAcquireResult, select_index_and_try_acquire},
};

pub(super) struct PlannedRequest {
    pub(super) decoded: DecodedRequest,
    pub(super) public_model: String,
    pub(super) selected: SelectedCandidate,
}

pub(super) fn plan(
    snapshot: &PublishedSnapshot,
    request: PublicRequest,
    adapter: &dyn ProtocolAdapter,
    providers: &ProviderRegistry,
) -> Result<PlannedRequest, PublicError> {
    let decoded = adapter
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: Uri::from_static("/"),
            headers: request.headers,
            body: request.body,
            operation: request.operation,
        })
        .map_err(|_| invalid_request("request body is not valid for this endpoint"))?;
    let public_model = decoded
        .model
        .as_deref()
        .ok_or_else(|| invalid_request("model is required"))
        .and_then(|model| {
            PublicModelName::new(model).map_err(|_| invalid_request("model name is invalid"))
        })?;
    let route = snapshot
        .model_routes()
        .resolve(decoded.dialect, &public_model)
        .filter(|route| route.enabled())
        .ok_or_else(|| public_error(PublicErrorCode::ModelNotFound, "model route was not found"))?;
    let transport_mode = if decoded.stream {
        TransportMode::Sse
    } else {
        TransportMode::Json
    };
    let tiers = build_route_candidates(snapshot, route, providers, transport_mode);
    let selected = select_candidate(
        snapshot,
        decoded.operation,
        route.id(),
        route.fallback_on_saturation(),
        tiers,
    )?;
    Ok(PlannedRequest {
        decoded,
        public_model: public_model.as_str().to_owned(),
        selected,
    })
}

fn select_candidate(
    snapshot: &PublishedSnapshot,
    operation: ProtocolOperation,
    route_id: any2api_domain::ModelRouteId,
    fallback_on_saturation: Option<bool>,
    tiers: BTreeMap<u16, Vec<RouteCandidate>>,
) -> Result<SelectedCandidate, PublicError> {
    if operation == ProtocolOperation::MessagesCountTokens {
        return select_auxiliary_candidate(snapshot, route_id, tiers);
    }
    select_generation_candidate(snapshot, route_id, fallback_on_saturation, tiers)
}

fn select_generation_candidate(
    snapshot: &PublishedSnapshot,
    route_id: any2api_domain::ModelRouteId,
    fallback_on_saturation: Option<bool>,
    tiers: BTreeMap<u16, Vec<RouteCandidate>>,
) -> Result<SelectedCandidate, PublicError> {
    let mut saw_capacity = false;
    for (tier, candidates) in tiers {
        let bindings = candidates
            .iter()
            .map(|candidate| candidate.binding.clone())
            .collect::<Vec<_>>();
        let tie_breaker = snapshot
            .route_tier_cursor(route_id, FallbackTier::new(tier))
            .ok_or_else(internal_error)?
            .reserve();
        match select_index_and_try_acquire(&bindings, tie_breaker) {
            IndexedSelectAndAcquireResult::Acquired { index, permit } => {
                return Ok(SelectedCandidate {
                    candidate: candidates[index].clone(),
                    permit: RequestPermit::Generation(permit),
                });
            }
            IndexedSelectAndAcquireResult::AtCapacity => {
                saw_capacity = true;
                if fallback_on_saturation != Some(true) {
                    return Err(public_error(
                        PublicErrorCode::LocalConcurrencyLimit,
                        "all eligible credentials are at capacity",
                    ));
                }
            }
            IndexedSelectAndAcquireResult::NoCandidates => {}
        }
    }
    if saw_capacity {
        Err(public_error(
            PublicErrorCode::LocalConcurrencyLimit,
            "all eligible credentials are at capacity",
        ))
    } else {
        Err(public_error(
            PublicErrorCode::NoAvailableCredential,
            "no eligible provider credential is available",
        ))
    }
}

fn select_auxiliary_candidate(
    snapshot: &PublishedSnapshot,
    route_id: any2api_domain::ModelRouteId,
    tiers: BTreeMap<u16, Vec<RouteCandidate>>,
) -> Result<SelectedCandidate, PublicError> {
    select_auxiliary_candidate_with(snapshot.auxiliary_scheduler(), tiers, |tier| {
        snapshot
            .route_tier_cursor(route_id, FallbackTier::new(tier))
            .map(|cursor| cursor.reserve())
    })
}

fn select_auxiliary_candidate_with(
    scheduler: &std::sync::Arc<crate::auxiliary_scheduler::AuxiliaryScheduler>,
    tiers: BTreeMap<u16, Vec<RouteCandidate>>,
    mut tie_breaker: impl FnMut(u16) -> Option<u64>,
) -> Result<SelectedCandidate, PublicError> {
    for (tier, candidates) in tiers {
        let bindings = candidates
            .iter()
            .map(|candidate| candidate.binding.clone())
            .collect::<Vec<_>>();
        let tie_breaker = tie_breaker(tier).ok_or_else(internal_error)?;
        match scheduler.select_index_and_try_acquire(&bindings, tie_breaker) {
            AuxiliarySelectAndAcquireResult::Acquired { index, permit } => {
                return Ok(SelectedCandidate {
                    candidate: candidates[index].clone(),
                    permit: RequestPermit::Auxiliary(permit),
                });
            }
            AuxiliarySelectAndAcquireResult::AtCapacity => {
                return Err(public_error(
                    PublicErrorCode::LocalConcurrencyLimit,
                    "auxiliary request capacity is full",
                ));
            }
            AuxiliarySelectAndAcquireResult::NoCandidates => {}
        }
    }
    Err(public_error(
        PublicErrorCode::NoAvailableCredential,
        "no eligible provider credential is available",
    ))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use any2api_domain::{
        CredentialId, CredentialKind, CredentialSecretFingerprint, MaxConcurrency,
        ProviderCredential, ProviderCredentialDraft, ProviderEndpointId, ProxyProfileId,
        PublicErrorCode, RouteTargetId,
    };

    use super::{RouteCandidate, select_auxiliary_candidate_with};
    use crate::{
        auxiliary_scheduler::{
            AuxiliaryConcurrencyLimits, AuxiliaryScheduler, AuxiliarySelectAndAcquireResult,
        },
        credential_auth::CredentialAuthMaterial,
        credential_runtime::CredentialRuntimeHandle,
        scheduler_epoch::SchedulerEpoch,
    };

    #[test]
    fn auxiliary_saturation_does_not_fall_through_to_a_later_tier() {
        let epoch = SchedulerEpoch::new();
        let scheduler = AuxiliaryScheduler::new(
            AuxiliaryConcurrencyLimits::new(1, 1).expect("limits"),
            Arc::clone(&epoch),
        );
        let primary = candidate("primary", 1, Arc::clone(&epoch), 0);
        let fallback = candidate("fallback", 2, Arc::clone(&epoch), 1);
        let primary_slot = match scheduler
            .select_index_and_try_acquire(std::slice::from_ref(&primary.binding), 0)
        {
            AuxiliarySelectAndAcquireResult::Acquired { permit, .. } => permit,
            AuxiliarySelectAndAcquireResult::AtCapacity => panic!("primary slot available"),
            AuxiliarySelectAndAcquireResult::NoCandidates => panic!("primary candidate exists"),
        };

        let tiers = BTreeMap::from([(0, vec![primary]), (1, vec![fallback.clone()])]);
        let error = match select_auxiliary_candidate_with(&scheduler, tiers, |_| Some(0)) {
            Ok(_) => panic!("primary saturation must fail immediately"),
            Err(error) => error,
        };

        assert_eq!(error.code, PublicErrorCode::LocalConcurrencyLimit);
        assert_eq!(fallback.binding.auxiliary_in_flight(), 0);
        drop(primary_slot);
    }

    fn candidate(
        label: &str,
        fingerprint_byte: u8,
        scheduler_epoch: Arc<SchedulerEpoch>,
        tier: u16,
    ) -> RouteCandidate {
        let credential = ProviderCredential::create(
            CredentialId::new(),
            ProviderEndpointId::new(),
            ProviderCredentialDraft::new(
                label,
                CredentialKind::ApiKey,
                ProxyProfileId::DIRECT,
                MaxConcurrency::new(1).expect("max concurrency"),
                true,
            )
            .expect("credential draft"),
            CredentialSecretFingerprint::new([fingerprint_byte; 32], None).expect("fingerprint"),
        );
        let binding = CredentialRuntimeHandle::new(
            &credential,
            CredentialAuthMaterial::for_test(&credential, format!("sk-{label}-test")),
            scheduler_epoch,
        )
        .current_binding();
        RouteCandidate {
            target_id: RouteTargetId::new(),
            endpoint_id: credential.provider_endpoint_id(),
            credential_id: credential.id(),
            upstream_model: format!("upstream-{tier}"),
            binding,
        }
    }
}
