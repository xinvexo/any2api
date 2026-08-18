use std::sync::Arc;

use any2api_domain::{
    ModelRouteId, ProtocolDialect, PublicError, PublicErrorCode, PublicModelName, TransportMode,
};
use any2api_protocol::api::IngressAffinity;
use any2api_protocol::api::{DecodedRequest, IngressRequest, ProtocolAdapter, ProtocolRegistry};
use any2api_provider::api::ProviderRegistry;
use http::{Method, Uri};

use super::{PublicRequest, response::invalid_request};
use crate::{
    configuration::PublishedSnapshot,
    routing::{CandidateRequirements, OAuthRoute, RouteCandidateTiers, oauth_route_id},
};

pub(super) struct PlannedRequest {
    pub(super) decoded: Arc<DecodedRequest>,
    pub(super) public_model: String,
    pub(super) route_id: ModelRouteId,
    pub(super) dialect: ProtocolDialect,
    pub(super) fallback_on_rate_limit: bool,
    pub(super) synthetic_oauth_route: bool,
    pub(super) tiers: Arc<RouteCandidateTiers>,
}

pub(super) struct DecodedPublicRequest {
    pub(super) decoded: DecodedRequest,
    pub(super) public_model: PublicModelName,
}

#[derive(Clone, Copy)]
pub(super) enum RoutingReplanMode {
    Unbound,
    FixedRoute,
}

impl RoutingReplanMode {
    pub(super) fn for_request(snapshot: &PublishedSnapshot, planned: &PlannedRequest) -> Self {
        match planned.decoded.affinity {
            IngressAffinity::None => Self::Unbound,
            IngressAffinity::Session(_) if !snapshot.affinity_policy().enabled() => Self::Unbound,
            IngressAffinity::Session(_) | IngressAffinity::Continuation(_) => Self::FixedRoute,
        }
    }
}

pub(super) async fn decode(
    request: PublicRequest,
    adapter: &dyn ProtocolAdapter,
) -> Result<DecodedPublicRequest, PublicError> {
    let decoded = adapter
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: Uri::from_static("/"),
            headers: request.headers,
            body: request.body,
            operation: request.operation,
        })
        .await
        .map_err(|_| invalid_request("request body is not valid for this endpoint"))?;
    let public_model = decoded
        .model
        .as_deref()
        .ok_or_else(|| invalid_request("model is required"))
        .and_then(|model| {
            PublicModelName::new(model).map_err(|_| invalid_request("model name is invalid"))
        })?;
    Ok(DecodedPublicRequest {
        decoded,
        public_model,
    })
}

pub(super) fn plan(
    snapshot: &PublishedSnapshot,
    request: DecodedPublicRequest,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
) -> Result<PlannedRequest, PublicError> {
    plan_decoded(
        snapshot,
        Arc::new(request.decoded),
        request.public_model,
        protocols,
        providers,
        snapshot.queue_policy().fallback_on_rate_limit(),
    )
}

pub(super) fn replan(
    snapshot: &PublishedSnapshot,
    planned: &PlannedRequest,
    mode: RoutingReplanMode,
    fallback_on_rate_limit: bool,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
) -> Result<PlannedRequest, PublicError> {
    let public_model = PublicModelName::new(planned.public_model.clone())
        .expect("planned public model was already validated");
    match mode {
        RoutingReplanMode::Unbound => plan_decoded(
            snapshot,
            Arc::clone(&planned.decoded),
            public_model,
            protocols,
            providers,
            fallback_on_rate_limit,
        ),
        RoutingReplanMode::FixedRoute => {
            replan_fixed_route(snapshot, planned, public_model, protocols, providers)
        }
    }
}

fn plan_decoded(
    snapshot: &PublishedSnapshot,
    decoded: Arc<DecodedRequest>,
    public_model: PublicModelName,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
    default_fallback_on_rate_limit: bool,
) -> Result<PlannedRequest, PublicError> {
    if !snapshot.is_public_model_allowed(&public_model) {
        return Err(model_not_found(&public_model));
    }
    let route = snapshot
        .model_routes()
        .resolve(decoded.dialect, &public_model)
        .filter(|route| route.enabled());
    let transport_mode = if decoded.stream {
        TransportMode::Sse
    } else {
        TransportMode::Json
    };
    let requirements =
        CandidateRequirements::new(decoded.operation, decoded.execution_profile, transport_mode);
    let (route_id, dialect, fallback_on_rate_limit, synthetic_oauth_route, tiers) =
        if let Some(route) = route {
            (
                route.id(),
                route.ingress_protocol(),
                route
                    .fallback_on_rate_limit()
                    .unwrap_or(default_fallback_on_rate_limit),
                false,
                snapshot.route_candidates(route, protocols, providers, requirements),
            )
        } else {
            let route_id = oauth_route_id(decoded.dialect, &public_model);
            let tiers = snapshot.oauth_route_candidates(
                OAuthRoute::new(route_id, decoded.dialect, &public_model),
                protocols,
                providers,
                requirements,
            );
            if tiers.is_empty() {
                return Err(model_not_found(&public_model));
            }
            (
                route_id,
                decoded.dialect,
                default_fallback_on_rate_limit,
                true,
                tiers,
            )
        };
    Ok(PlannedRequest {
        decoded,
        public_model: public_model.as_str().to_owned(),
        route_id,
        dialect,
        fallback_on_rate_limit,
        synthetic_oauth_route,
        tiers,
    })
}

fn replan_fixed_route(
    snapshot: &PublishedSnapshot,
    planned: &PlannedRequest,
    public_model: PublicModelName,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
) -> Result<PlannedRequest, PublicError> {
    if !snapshot.is_public_model_allowed(&public_model) {
        return Err(model_not_found(&public_model));
    }
    let transport_mode = if planned.decoded.stream {
        TransportMode::Sse
    } else {
        TransportMode::Json
    };
    let requirements = CandidateRequirements::new(
        planned.decoded.operation,
        planned.decoded.execution_profile,
        transport_mode,
    );
    let tiers = if let Some(route) = snapshot
        .model_routes()
        .get(planned.route_id)
        .filter(|route| route.enabled() && route.ingress_protocol() == planned.dialect)
    {
        snapshot.route_candidates(route, protocols, providers, requirements)
    } else if planned.synthetic_oauth_route {
        snapshot.oauth_route_candidates(
            OAuthRoute::new(planned.route_id, planned.dialect, &public_model),
            protocols,
            providers,
            requirements,
        )
    } else {
        Arc::new(RouteCandidateTiers::new())
    };
    Ok(PlannedRequest {
        decoded: Arc::clone(&planned.decoded),
        public_model: planned.public_model.clone(),
        route_id: planned.route_id,
        dialect: planned.dialect,
        fallback_on_rate_limit: planned.fallback_on_rate_limit,
        synthetic_oauth_route: planned.synthetic_oauth_route,
        tiers,
    })
}

fn model_not_found(model: &PublicModelName) -> PublicError {
    PublicError::new(
        PublicErrorCode::ModelNotFound,
        format!(
            "The model '{}' is not available through this gateway.",
            model.as_str()
        ),
    )
}
