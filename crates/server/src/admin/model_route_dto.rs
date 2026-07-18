use std::collections::HashSet;

use any2api_domain::{
    ConfigRevision, FallbackTier, ModelRoute, ModelRouteDraft, ModelRouteId, ProtocolDialect,
    ProviderEndpointId, RouteTargetDraft, RouteTargetId,
};
use any2api_runtime::api::PublishedSnapshot;
use serde::{Deserialize, Serialize};

use super::{error::AdminApiError, revision::parse_revision};

#[derive(Debug, Serialize)]
pub(crate) struct ModelRouteCollectionResponse {
    config_revision: u64,
    items: Vec<ModelRouteResponse>,
}

impl ModelRouteCollectionResponse {
    pub(crate) fn from_snapshot(snapshot: &PublishedSnapshot) -> Self {
        Self {
            config_revision: snapshot.revision().get(),
            items: snapshot
                .model_routes()
                .routes()
                .iter()
                .map(ModelRouteResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ModelRouteResponse {
    id: ModelRouteId,
    public_model: String,
    ingress_protocol: ProtocolDialect,
    fallback_on_saturation: Option<bool>,
    enabled: bool,
    config_version: u64,
    targets: Vec<RouteTargetResponse>,
}

impl From<&ModelRoute> for ModelRouteResponse {
    fn from(route: &ModelRoute) -> Self {
        Self {
            id: route.id(),
            public_model: route.public_model().as_str().to_owned(),
            ingress_protocol: route.ingress_protocol(),
            fallback_on_saturation: route.fallback_on_saturation(),
            enabled: route.enabled(),
            config_version: route.config_version(),
            targets: route
                .targets()
                .iter()
                .map(RouteTargetResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct RouteTargetResponse {
    id: RouteTargetId,
    provider_endpoint_id: ProviderEndpointId,
    upstream_model: String,
    fallback_tier: u16,
    enabled: bool,
}

impl From<&any2api_domain::RouteTarget> for RouteTargetResponse {
    fn from(target: &any2api_domain::RouteTarget) -> Self {
        Self {
            id: target.id(),
            provider_endpoint_id: target.provider_endpoint_id(),
            upstream_model: target.upstream_model().as_str().to_owned(),
            fallback_tier: target.fallback_tier().get(),
            enabled: target.enabled(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelRouteWriteRequest {
    expected_revision: u64,
    expected_config_version: Option<u64>,
    public_model: String,
    ingress_protocol: ProtocolDialect,
    fallback_on_saturation: Option<bool>,
    enabled: bool,
    targets: Vec<RouteTargetWriteRequest>,
}

impl ModelRouteWriteRequest {
    pub(crate) fn revision(&self) -> Result<ConfigRevision, AdminApiError> {
        parse_revision(self.expected_revision)
    }

    pub(crate) fn into_create_domain(
        self,
    ) -> Result<(ConfigRevision, ModelRouteDraft), AdminApiError> {
        let revision = self.revision()?;
        let Self {
            expected_revision: _,
            expected_config_version,
            public_model,
            ingress_protocol,
            fallback_on_saturation,
            enabled,
            targets,
        } = self;
        if expected_config_version.is_some() {
            return Err(AdminApiError::invalid_request(
                "expected_config_version is only valid for updates",
            ));
        }
        if targets.iter().any(|target| target.id.is_some()) {
            return Err(AdminApiError::invalid_request(
                "new route targets must not include an id",
            ));
        }
        let targets = build_targets(targets, None)?;
        let draft = build_route_draft(
            public_model,
            ingress_protocol,
            fallback_on_saturation,
            enabled,
            targets,
        )?;
        Ok((revision, draft))
    }

    pub(crate) fn into_update_domain(
        self,
        existing: &ModelRoute,
    ) -> Result<(ConfigRevision, u64, ModelRouteDraft), AdminApiError> {
        let revision = self.revision()?;
        let Self {
            expected_revision: _,
            expected_config_version,
            public_model,
            ingress_protocol,
            fallback_on_saturation,
            enabled,
            targets,
        } = self;
        let expected_config_version = expected_config_version
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                AdminApiError::invalid_request("expected_config_version is required for updates")
            })?;
        let existing_targets = ExistingTargets::from_route(existing);
        let targets = build_targets(targets, Some(&existing_targets))?;
        let draft = build_route_draft(
            public_model,
            ingress_protocol,
            fallback_on_saturation,
            enabled,
            targets,
        )?;
        Ok((revision, expected_config_version, draft))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteTargetWriteRequest {
    id: Option<RouteTargetId>,
    provider_endpoint_id: ProviderEndpointId,
    upstream_model: String,
    fallback_tier: u16,
    enabled: bool,
}

struct ExistingTargets {
    ids: HashSet<RouteTargetId>,
    identities: HashSet<(ProviderEndpointId, String)>,
}

impl ExistingTargets {
    fn from_route(route: &ModelRoute) -> Self {
        Self {
            ids: route.targets().iter().map(|target| target.id()).collect(),
            identities: route
                .targets()
                .iter()
                .map(|target| {
                    (
                        target.provider_endpoint_id(),
                        target.upstream_model().as_str().to_owned(),
                    )
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ModelRouteDeleteQuery {
    expected_revision: u64,
    expected_config_version: u64,
}

impl ModelRouteDeleteQuery {
    pub(crate) fn into_domain(self) -> Result<(ConfigRevision, u64), AdminApiError> {
        if self.expected_config_version == 0 {
            return Err(AdminApiError::invalid_request(
                "expected_config_version is invalid",
            ));
        }
        Ok((
            parse_revision(self.expected_revision)?,
            self.expected_config_version,
        ))
    }
}

fn build_targets(
    targets: Vec<RouteTargetWriteRequest>,
    existing_targets: Option<&ExistingTargets>,
) -> Result<Vec<RouteTargetDraft>, AdminApiError> {
    targets
        .into_iter()
        .map(|target| {
            let id = match target.id {
                Some(id) => {
                    if !existing_targets.is_some_and(|existing| existing.ids.contains(&id)) {
                        return Err(AdminApiError::invalid_request(
                            "route target id is unknown for this route",
                        ));
                    }
                    id
                }
                None => {
                    let identity = (target.provider_endpoint_id, target.upstream_model.clone());
                    if existing_targets
                        .is_some_and(|existing| existing.identities.contains(&identity))
                    {
                        return Err(AdminApiError::invalid_request(
                            "an existing route target identity must include its id",
                        ));
                    }
                    RouteTargetId::new()
                }
            };
            RouteTargetDraft::new(
                id,
                target.provider_endpoint_id,
                target.upstream_model,
                FallbackTier::new(target.fallback_tier),
                target.enabled,
            )
            .map_err(|error| AdminApiError::invalid_model_route(error.to_string()))
        })
        .collect()
}

fn build_route_draft(
    public_model: String,
    ingress_protocol: ProtocolDialect,
    fallback_on_saturation: Option<bool>,
    enabled: bool,
    targets: Vec<RouteTargetDraft>,
) -> Result<ModelRouteDraft, AdminApiError> {
    ModelRouteDraft::new(
        public_model,
        ingress_protocol,
        fallback_on_saturation,
        enabled,
        targets,
    )
    .map_err(|error| AdminApiError::invalid_model_route(error.to_string()))
}
