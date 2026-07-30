use any2api_domain::{
    ModelRouteId, ProtocolDialect, PublicError, PublicErrorCode, PublicModelName, TransportMode,
};
use any2api_protocol::api::{DecodedRequest, IngressRequest, ProtocolAdapter, ProtocolRegistry};
use any2api_provider::api::ProviderRegistry;
use http::{Method, Uri};

use super::{PublicRequest, response::invalid_request};
use crate::{
    configuration::PublishedSnapshot,
    routing::{
        CandidateRequirements, OAuthRoute, build_oauth_route_candidates, build_route_candidates,
        oauth_route_id,
    },
};

pub(super) struct PlannedRequest {
    pub(super) decoded: DecodedRequest,
    pub(super) public_model: String,
    pub(super) route_id: ModelRouteId,
    pub(super) dialect: ProtocolDialect,
    pub(super) fallback_on_rate_limit: bool,
    pub(super) tiers: std::collections::BTreeMap<u16, Vec<crate::routing::RouteCandidate>>,
}

pub(super) struct DecodedPublicRequest {
    pub(super) decoded: DecodedRequest,
    pub(super) public_model: PublicModelName,
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
        request.decoded,
        request.public_model,
        protocols,
        providers,
    )
}

pub(super) fn replan(
    snapshot: &PublishedSnapshot,
    planned: &PlannedRequest,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
) -> Result<PlannedRequest, PublicError> {
    let public_model = PublicModelName::new(planned.public_model.clone())
        .expect("planned public model was already validated");
    plan_decoded(
        snapshot,
        planned.decoded.clone(),
        public_model,
        protocols,
        providers,
    )
}

fn plan_decoded(
    snapshot: &PublishedSnapshot,
    decoded: DecodedRequest,
    public_model: PublicModelName,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
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
    let (route_id, dialect, fallback_on_rate_limit, tiers) = if let Some(route) = route {
        (
            route.id(),
            route.ingress_protocol(),
            route
                .fallback_on_rate_limit()
                .unwrap_or_else(|| snapshot.queue_policy().fallback_on_rate_limit()),
            build_route_candidates(snapshot, route, protocols, providers, requirements),
        )
    } else {
        let route_id = oauth_route_id(decoded.dialect, &public_model);
        let tiers = build_oauth_route_candidates(
            snapshot,
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
            snapshot.queue_policy().fallback_on_rate_limit(),
            tiers,
        )
    };
    Ok(PlannedRequest {
        decoded,
        public_model: public_model.as_str().to_owned(),
        route_id,
        dialect,
        fallback_on_rate_limit,
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
