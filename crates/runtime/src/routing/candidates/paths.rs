use std::collections::HashSet;

use any2api_domain::{ModelRouteConfiguration, ProxyConfiguration, PublicModelName};

use super::oauth::{oauth_target_id, resolved_oauth_route_id};
use crate::{
    health::CandidatePathBaseKey,
    routing::{RoutingCredential, RoutingCredentials},
};

pub(crate) fn active_candidate_path_bases(
    routes: &ModelRouteConfiguration,
    credentials: &RoutingCredentials,
    proxies: &ProxyConfiguration,
) -> HashSet<CandidatePathBaseKey> {
    let mut paths = HashSet::new();
    for route in routes.routes().iter().filter(|route| route.enabled()) {
        for target in route.targets().iter().filter(|target| target.enabled()) {
            for credential in credentials
                .as_slice()
                .iter()
                .filter(|credential| !credential.is_oauth())
                .filter(|credential| candidate_credential_is_active(credential, proxies))
                .filter(|credential| credential.endpoint_id() == target.provider_endpoint_id())
                .filter(|credential| credential.ingress_protocol() == route.ingress_protocol())
                .filter(|credential| {
                    credential.upstream_protocol() == target.upstream_protocol_dialect()
                })
                .filter(|credential| credential.supports_model(target.upstream_model()))
            {
                paths.insert(path_base(target.id(), credential));
            }
        }
    }

    for credential in credentials
        .as_slice()
        .iter()
        .filter(|credential| credential.is_oauth())
        .filter(|credential| candidate_credential_is_active(credential, proxies))
    {
        for model in credential.models() {
            let Ok(public_model) = PublicModelName::new(model.as_str().to_owned()) else {
                continue;
            };
            let route_id =
                resolved_oauth_route_id(routes, credential.ingress_protocol(), &public_model);
            let target_id = oauth_target_id(
                route_id,
                credential.endpoint_id(),
                credential.upstream_protocol(),
            );
            paths.insert(path_base(target_id, credential));
        }
    }
    paths
}

fn candidate_credential_is_active(
    credential: &RoutingCredential,
    proxies: &ProxyConfiguration,
) -> bool {
    credential.routable()
        && proxies
            .get(credential.proxy_id())
            .is_some_and(|proxy| proxy.enabled())
}

fn path_base(
    target_id: any2api_domain::RouteTargetId,
    credential: &RoutingCredential,
) -> CandidatePathBaseKey {
    CandidatePathBaseKey {
        target_id,
        credential_id: credential.id(),
        routing_generation: credential.binding().generation().routing_generation(),
        endpoint_id: credential.endpoint_id(),
        endpoint_config_version: credential.endpoint_config_version(),
        proxy_id: credential.proxy_id(),
        proxy_config_version: credential.proxy_config_version(),
    }
}
