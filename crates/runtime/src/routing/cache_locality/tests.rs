use any2api_domain::{
    ModelRouteId, ProtocolDialect, ProtocolOperation, ProviderEndpointId, ProxyProfileId,
    RouteTargetId, RoutingCredentialId,
};

use super::{
    CACHE_LOCALITY_TTL, CacheLocalityCompletion, CacheLocalityRegistry, CacheLocalityTarget,
};
use crate::routing::CandidateIdentity;

#[tokio::test(start_paused = true)]
async fn entries_are_scoped_sliding_and_expire() {
    let registry = CacheLocalityRegistry::new();
    let route = ModelRouteId::new();
    let key = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        route,
        "same-cache-key",
    );
    let other_route = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        ModelRouteId::new(),
        "same-cache-key",
    );
    let other_operation = registry.key(
        ProtocolDialect::OpenAiChatCompletions,
        ProtocolOperation::ChatCompletions,
        route,
        "same-cache-key",
    );
    let target = target(1);
    registry.remember_target(key, target.clone());

    assert_eq!(registry.lookup(key), Some(target.clone()));
    assert_eq!(registry.lookup(other_route), None);
    assert_eq!(registry.lookup(other_operation), None);
    tokio::time::advance(CACHE_LOCALITY_TTL - std::time::Duration::from_secs(1)).await;
    assert_eq!(registry.lookup(key), Some(target));
    tokio::time::advance(CACHE_LOCALITY_TTL).await;
    assert_eq!(registry.lookup(key), None);
}

#[tokio::test(start_paused = true)]
async fn expired_entries_free_capacity_without_evicting_live_entries() {
    let registry = CacheLocalityRegistry::with_capacity_for_test(2);
    let route = ModelRouteId::new();
    let expired = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        route,
        "expired",
    );
    let live = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        route,
        "live",
    );
    let added = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        route,
        "added",
    );
    registry.remember_target(expired, target(1));
    tokio::time::advance(CACHE_LOCALITY_TTL / 2).await;
    registry.remember_target(live, target(2));
    tokio::time::advance(CACHE_LOCALITY_TTL / 2 + std::time::Duration::from_secs(1)).await;
    registry.remember_target(added, target(3));

    assert_eq!(registry.len_for_test(), 2);
    assert!(registry.lookup(expired).is_none());
    assert!(registry.lookup(live).is_some());
    assert!(registry.lookup(added).is_some());
}

#[test]
fn compare_delete_does_not_remove_a_newer_target() {
    let registry = CacheLocalityRegistry::new();
    let key = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        ModelRouteId::new(),
        "cache-key",
    );
    let old = target(1);
    let current = target(2);
    registry.remember_target(key, current.clone());
    registry.forget_target(key, &old);

    assert_eq!(registry.lookup(key), Some(current));
}

#[test]
fn capacity_evicts_the_least_recently_used_entry() {
    let registry = CacheLocalityRegistry::with_capacity_for_test(2);
    let route = ModelRouteId::new();
    let first = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        route,
        "first",
    );
    let second = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        route,
        "second",
    );
    let third = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        route,
        "third",
    );
    registry.remember_target(first, target(1));
    registry.remember_target(second, target(2));
    assert!(registry.lookup(first).is_some());
    registry.remember_target(third, target(3));

    assert_eq!(registry.len_for_test(), 2);
    assert!(registry.lookup(first).is_some());
    assert!(registry.lookup(second).is_none());
    assert!(registry.lookup(third).is_some());
}

#[test]
fn completion_only_changes_the_hint_when_explicitly_settled() {
    let registry = CacheLocalityRegistry::new();
    let key = registry.key(
        ProtocolDialect::OpenAiResponses,
        ProtocolOperation::Responses,
        ModelRouteId::new(),
        "cache-key",
    );
    let previous = target(1);
    let current = target(2);
    registry.remember_target(key, previous.clone());

    drop(CacheLocalityCompletion {
        registry: registry.clone(),
        key,
        target: current.clone(),
    });
    assert_eq!(registry.lookup(key), Some(previous.clone()));

    CacheLocalityCompletion {
        registry: registry.clone(),
        key,
        target: current.clone(),
    }
    .success();
    assert_eq!(registry.lookup(key), Some(current.clone()));

    CacheLocalityCompletion {
        registry: registry.clone(),
        key,
        target: previous,
    }
    .failure();
    assert_eq!(registry.lookup(key), Some(current.clone()));

    CacheLocalityCompletion {
        registry: registry.clone(),
        key,
        target: current,
    }
    .failure();
    assert_eq!(registry.lookup(key), None);
}

fn target(marker: u64) -> CacheLocalityTarget {
    CacheLocalityTarget {
        identity: CandidateIdentity {
            target_id: RouteTargetId::new(),
            operation: ProtocolOperation::Responses,
            credential_id: RoutingCredentialId::provider_credential(
                any2api_domain::CredentialId::new(),
            ),
            routing_generation: marker,
            endpoint_id: ProviderEndpointId::new(),
            endpoint_config_version: marker,
            proxy_id: ProxyProfileId::DIRECT,
            proxy_config_version: marker,
        },
        upstream_model: format!("model-{marker}"),
        upstream_protocol_dialect: ProtocolDialect::OpenAiResponses,
    }
}
