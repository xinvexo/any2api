use any2api_domain::{
    ModelRouteId, ProtocolDialect, PublicError, PublicErrorCode, PublicModelName, TransportMode,
};
use any2api_protocol::api::{DecodedRequest, IngressRequest, ProtocolAdapter, ProtocolRegistry};
use any2api_provider::api::ProviderRegistry;
use http::{Method, Uri};

use super::{
    PublicRequest,
    response::{invalid_request, public_error},
};
use crate::{published_snapshot::PublishedSnapshot, route_candidates::build_route_candidates};

pub(super) struct PlannedRequest {
    pub(super) decoded: DecodedRequest,
    pub(super) public_model: String,
    pub(super) route_id: ModelRouteId,
    pub(super) dialect: ProtocolDialect,
    pub(super) fallback_on_saturation: bool,
    pub(super) tiers: std::collections::BTreeMap<u16, Vec<crate::route_candidates::RouteCandidate>>,
}

pub(super) async fn plan(
    snapshot: &PublishedSnapshot,
    request: PublicRequest,
    adapter: &dyn ProtocolAdapter,
    protocols: &ProtocolRegistry,
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
    let tiers = build_route_candidates(
        snapshot,
        route,
        protocols,
        providers,
        decoded.operation,
        transport_mode,
    );
    let fallback_on_saturation = route
        .fallback_on_saturation()
        .unwrap_or_else(|| snapshot.queue_policy().fallback_on_saturation());
    Ok(PlannedRequest {
        decoded,
        public_model: public_model.as_str().to_owned(),
        route_id: route.id(),
        dialect: route.ingress_protocol(),
        fallback_on_saturation,
        tiers,
    })
}
