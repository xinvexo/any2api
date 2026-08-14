use std::collections::{BTreeMap, HashSet};

use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicModelName, TransportMode};
use any2api_protocol::api::{ProtocolRegistry, RequestExecutionProfile};
use any2api_provider::api::ProviderRegistry;

use super::{CandidateRequirements, OAuthRoute, oauth_route_id};
use crate::configuration::PublishedSnapshot;

mod model;

pub use model::{
    RouteInspectionCandidateGroup, RouteInspectionItem, RouteInspectionOperation,
    RouteInspectionSnapshot, RouteInspectionStatus,
};

pub(crate) fn inspect_routes(
    snapshot: &PublishedSnapshot,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
) -> RouteInspectionSnapshot {
    let mut keys = BTreeMap::<(PublicModelName, ProtocolDialect), bool>::new();
    for route in snapshot.model_routes().routes() {
        keys.entry((route.public_model().clone(), route.ingress_protocol()))
            .or_default();
    }
    for credential in snapshot
        .routing_credentials()
        .iter()
        .filter(|credential| credential.is_oauth())
    {
        for model in credential.models() {
            let Ok(public_model) = PublicModelName::new(model.as_str().to_owned()) else {
                continue;
            };
            keys.insert((public_model, credential.ingress_protocol()), true);
        }
    }

    let items = keys
        .into_iter()
        .filter(|((public_model, _), _)| snapshot.is_public_model_allowed(public_model))
        .map(|((public_model, ingress_protocol), oauth_published)| {
            inspect_item(
                snapshot,
                protocols,
                providers,
                public_model,
                ingress_protocol,
                oauth_published,
            )
        })
        .collect();
    RouteInspectionSnapshot {
        config_revision: snapshot.revision(),
        items,
    }
}

fn inspect_item(
    snapshot: &PublishedSnapshot,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
    public_model: PublicModelName,
    ingress_protocol: ProtocolDialect,
    oauth_published: bool,
) -> RouteInspectionItem {
    let route = snapshot
        .model_routes()
        .resolve(ingress_protocol, &public_model);
    let published = oauth_published || route.is_some_and(any2api_domain::ModelRoute::enabled);
    let operations = ProtocolOperation::ALL
        .into_iter()
        .filter(|operation| operation.dialect() == ingress_protocol)
        .map(|operation| {
            inspect_operation(
                snapshot,
                protocols,
                providers,
                route,
                &public_model,
                ingress_protocol,
                operation,
            )
        })
        .collect::<Vec<_>>();
    let has_candidate = operations
        .iter()
        .any(|operation| !operation.candidate_groups.is_empty());
    let status = if has_candidate {
        RouteInspectionStatus::Available
    } else {
        RouteInspectionStatus::NoEnabledCandidate
    };
    RouteInspectionItem {
        public_model: public_model.as_str().to_owned(),
        ingress_protocol,
        published,
        status,
        operations,
    }
}

fn inspect_operation(
    snapshot: &PublishedSnapshot,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
    route: Option<&any2api_domain::ModelRoute>,
    public_model: &PublicModelName,
    ingress_protocol: ProtocolDialect,
    operation: ProtocolOperation,
) -> RouteInspectionOperation {
    let mut seen = HashSet::new();
    let mut groups = BTreeMap::new();
    for transport_mode in transport_modes(operation) {
        let requirements =
            CandidateRequirements::new(operation, execution_profile(operation), transport_mode);
        let tiers = match route.filter(|route| route.enabled()) {
            Some(route) => snapshot.route_candidates(route, protocols, providers, requirements),
            None => {
                let route_id = oauth_route_id(ingress_protocol, public_model);
                snapshot.oauth_route_candidates(
                    OAuthRoute::new(route_id, ingress_protocol, public_model),
                    protocols,
                    providers,
                    requirements,
                )
            }
        };
        for candidate in tiers.values().flatten() {
            if !seen.insert(candidate.identity()) {
                continue;
            }
            let endpoint_id = candidate
                .credential_id
                .provider_credential_id()
                .map(|_| candidate.endpoint_id);
            *groups
                .entry((
                    candidate.provider_kind,
                    endpoint_id,
                    candidate.upstream_protocol_dialect,
                ))
                .or_insert(0usize) += 1;
        }
    }
    let candidate_groups = groups
        .into_iter()
        .map(
            |((provider_kind, provider_endpoint_id, upstream_protocol_dialect), count)| {
                RouteInspectionCandidateGroup {
                    provider_kind,
                    provider_endpoint_id,
                    provider_endpoint_name: provider_endpoint_id.and_then(|id| {
                        snapshot
                            .provider_endpoints()
                            .get(id)
                            .map(|endpoint| endpoint.name().to_owned())
                    }),
                    upstream_protocol_dialect,
                    enabled_candidate_count: count,
                }
            },
        )
        .collect();
    RouteInspectionOperation {
        operation,
        candidate_groups,
    }
}

fn execution_profile(operation: ProtocolOperation) -> RequestExecutionProfile {
    if operation == ProtocolOperation::ResponsesCompact {
        RequestExecutionProfile::RemoteCompaction
    } else {
        RequestExecutionProfile::Standard
    }
}

fn transport_modes(operation: ProtocolOperation) -> impl Iterator<Item = TransportMode> {
    [TransportMode::Json, TransportMode::Sse]
        .into_iter()
        .filter(move |mode| *mode == TransportMode::Json || operation.allows_stream())
}

#[cfg(test)]
mod tests;
