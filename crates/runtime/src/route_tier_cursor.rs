use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, RwLock},
};

use any2api_domain::{FallbackTier, ModelRouteConfiguration, ModelRouteId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct RouteTierKey {
    route_id: ModelRouteId,
    tier: FallbackTier,
}

#[derive(Debug, Default)]
struct RouteTierCursor {
    next: AtomicU64,
}

impl RouteTierCursor {
    fn reserve(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RouteTierCursorBinding {
    cursor: Arc<RouteTierCursor>,
}

impl RouteTierCursorBinding {
    pub(crate) fn reserve(&self) -> u64 {
        self.cursor.reserve()
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RouteTierCursorBindings {
    cursors: HashMap<RouteTierKey, RouteTierCursorBinding>,
}

impl RouteTierCursorBindings {
    pub(crate) fn get(
        &self,
        route_id: ModelRouteId,
        tier: FallbackTier,
    ) -> Option<&RouteTierCursorBinding> {
        self.cursors.get(&RouteTierKey { route_id, tier })
    }
}

#[derive(Debug, Default)]
pub(crate) struct RouteTierCursorRegistry {
    cursors: RwLock<HashMap<RouteTierKey, Arc<RouteTierCursor>>>,
}

impl RouteTierCursorRegistry {
    pub(crate) fn reconcile(
        &self,
        configuration: &ModelRouteConfiguration,
    ) -> RouteTierCursorBindings {
        let active_keys = configuration
            .routes()
            .iter()
            .flat_map(|route| {
                route.targets().iter().map(move |target| RouteTierKey {
                    route_id: route.id(),
                    tier: target.fallback_tier(),
                })
            })
            .collect::<HashSet<_>>();
        let mut cursors = self
            .cursors
            .write()
            .expect("route tier cursor registry lock poisoned");
        cursors.retain(|key, _| active_keys.contains(key));

        let mut bindings = HashMap::with_capacity(active_keys.len());
        for key in active_keys {
            let cursor = cursors
                .entry(key)
                .or_insert_with(|| Arc::new(RouteTierCursor::default()))
                .clone();
            bindings.insert(key, RouteTierCursorBinding { cursor });
        }
        RouteTierCursorBindings { cursors: bindings }
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{
        FallbackTier, ModelRoute, ModelRouteConfiguration, ModelRouteDraft, ModelRouteId,
        ProtocolDialect, ProviderEndpoint, ProviderEndpointConfiguration, ProviderEndpointDraft,
        ProviderEndpointId, ProviderKind, RouteTargetDraft, RouteTargetId,
    };

    use super::RouteTierCursorRegistry;

    #[test]
    fn cursors_are_isolated_and_reused_until_a_route_tier_is_removed() {
        let endpoint = endpoint();
        let endpoints = ProviderEndpointConfiguration::new(vec![endpoint.clone()])
            .expect("endpoint configuration");
        let route_id = ModelRouteId::new();
        let route = route(route_id, endpoint.id(), 0);
        let configuration =
            ModelRouteConfiguration::new(vec![route], &endpoints).expect("route configuration");
        let registry = RouteTierCursorRegistry::default();

        let first = registry.reconcile(&configuration);
        let first_tier = first
            .get(route_id, FallbackTier::new(0))
            .expect("tier cursor");
        assert_eq!(first_tier.reserve(), 0);
        assert_eq!(first_tier.reserve(), 1);

        let second = registry.reconcile(&configuration);
        let second_tier = second
            .get(route_id, FallbackTier::new(0))
            .expect("tier cursor");
        assert_eq!(second_tier.reserve(), 2);

        let empty = ModelRouteConfiguration::initial();
        let removed = registry.reconcile(&empty);
        assert!(removed.get(route_id, FallbackTier::new(0)).is_none());

        let restored = registry.reconcile(&configuration);
        let restored_tier = restored
            .get(route_id, FallbackTier::new(0))
            .expect("new tier cursor");
        assert_eq!(restored_tier.reserve(), 0);
    }

    #[test]
    fn separate_tiers_have_independent_sequences() {
        let endpoint = endpoint();
        let endpoints = ProviderEndpointConfiguration::new(vec![endpoint.clone()])
            .expect("endpoint configuration");
        let route_id = ModelRouteId::new();
        let configuration = ModelRouteConfiguration::new(
            vec![route_with_tiers(route_id, endpoint.id(), &[0, 1])],
            &endpoints,
        )
        .expect("route configuration");
        let registry = RouteTierCursorRegistry::default();
        let bindings = registry.reconcile(&configuration);

        let primary = bindings
            .get(route_id, FallbackTier::new(0))
            .expect("primary cursor");
        let fallback = bindings
            .get(route_id, FallbackTier::new(1))
            .expect("fallback cursor");
        assert_eq!(primary.reserve(), 0);
        assert_eq!(fallback.reserve(), 0);
        assert_eq!(primary.reserve(), 1);
        assert_eq!(fallback.reserve(), 1);
    }

    fn endpoint() -> ProviderEndpoint {
        ProviderEndpoint::create(
            ProviderEndpointId::new(),
            ProviderEndpointDraft::new(
                "codex",
                ProviderKind::Codex,
                "https://api.example.com",
                ProtocolDialect::OpenAiResponses,
                false,
                false,
                true,
            )
            .expect("endpoint draft"),
        )
        .expect("endpoint")
    }

    fn route(route_id: ModelRouteId, endpoint_id: ProviderEndpointId, tier: u16) -> ModelRoute {
        route_with_tiers(route_id, endpoint_id, &[tier])
    }

    fn route_with_tiers(
        route_id: ModelRouteId,
        endpoint_id: ProviderEndpointId,
        tiers: &[u16],
    ) -> ModelRoute {
        ModelRoute::create(
            route_id,
            ModelRouteDraft::new(
                format!("model-{}", tiers[0]),
                ProtocolDialect::OpenAiResponses,
                None,
                true,
                tiers
                    .iter()
                    .map(|tier| {
                        RouteTargetDraft::new(
                            RouteTargetId::new(),
                            endpoint_id,
                            format!("upstream-{tier}"),
                            FallbackTier::new(*tier),
                            true,
                        )
                        .expect("target draft")
                    })
                    .collect(),
            )
            .expect("route draft"),
        )
    }
}
