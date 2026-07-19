use std::collections::BTreeMap;

use any2api_domain::{FallbackTier, PublicError, PublicErrorCode, PublicModelName};
use any2api_protocol::api::{DecodedRequest, IngressRequest, ProtocolAdapter};
use any2api_provider::api::ProviderRegistry;
use http::{Method, Uri};

use super::{
    PublicRequest, SelectedCandidate,
    response::{internal_error, invalid_request, public_error},
};
use crate::{
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
    if decoded.stream {
        return Err(invalid_request("streaming is not implemented yet"));
    }
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
    let tiers = build_route_candidates(snapshot, route, providers);
    let selected = select_candidate(snapshot, route.id(), route.fallback_on_saturation(), tiers)?;
    Ok(PlannedRequest {
        decoded,
        public_model: public_model.as_str().to_owned(),
        selected,
    })
}

fn select_candidate(
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
                    permit,
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
