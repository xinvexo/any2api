use std::time::{Duration, Instant};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect};

use super::{TTL, target};
use crate::affinity::{AffinityRegistry, BindingStart, registry::BindingState};

#[test]
fn expired_session_and_continuation_bindings_are_not_reused() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-expired",
            TTL,
        )
        .expect("session lease")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    lease
        .commit(target.clone())
        .expect("commit session binding");
    assert!(matches!(
        registry
            .begin_session(
                ProtocolDialect::OpenAiResponses,
                route_id,
                "session-expired",
                Duration::ZERO,
            )
            .expect("expired session binding is replaced"),
        BindingStart::Create(_)
    ));

    registry
        .bind_continuation("resp-expired", target, TTL)
        .expect("continuation binding");
    assert!(
        registry
            .resolve_continuation("resp-expired", Duration::ZERO)
            .is_none()
    );
}

#[test]
fn sweep_uses_one_ttl_for_session_and_continuation_bindings() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-sweep",
            TTL,
        )
        .expect("session binding")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    lease
        .commit(target.clone())
        .expect("commit session binding");
    registry
        .bind_continuation("resp-sweep", target, TTL)
        .expect("continuation binding");

    assert_eq!(registry.sweep_expired(TTL), 0);
    assert_eq!(registry.sweep_expired(Duration::ZERO), 2);
    assert!(registry.resolve_continuation("resp-sweep", TTL).is_none());
}

#[test]
fn session_and_continuation_access_refresh_the_same_ttl() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-refresh",
            TTL,
        )
        .expect("session binding")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    lease
        .commit(target.clone())
        .expect("commit session binding");
    registry
        .bind_continuation("resp-refresh", target, TTL)
        .expect("continuation binding");

    let stale_at = Instant::now() - Duration::from_secs(60);
    {
        let mut state = registry.state.lock().expect("affinity state");
        for entry in state.entries.values_mut() {
            let BindingState::Bound { binding } = entry else {
                panic!("all test entries are bound");
            };
            binding.last_seen_at = stale_at;
        }
    }

    assert!(matches!(
        registry
            .begin_session(
                ProtocolDialect::OpenAiResponses,
                route_id,
                "session-refresh",
                TTL,
            )
            .expect("refresh session binding"),
        BindingStart::Bound(_)
    ));
    registry
        .resolve_continuation("resp-refresh", TTL)
        .expect("refresh continuation binding");

    let state = registry.state.lock().expect("affinity state");
    assert!(state.entries.values().all(|entry| match entry {
        BindingState::Bound { binding } => binding.last_seen_at > stale_at,
        BindingState::Creating { .. } => false,
    }));
}

#[test]
fn snapshots_are_unified_redacted_and_cleanup_is_scoped() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let credential_id = CredentialId::new();
    let target = target(route_id, credential_id);
    let lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "private-session-id",
            TTL,
        )
        .expect("session binding")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    lease
        .commit(target.clone())
        .expect("commit session binding");
    registry
        .bind_continuation("private-response-id", target, TTL)
        .expect("continuation binding");
    let creating_route = ModelRouteId::new();
    let _creating = registry
        .begin_session(
            ProtocolDialect::AnthropicMessages,
            creating_route,
            "creating-session",
            TTL,
        )
        .expect("creating binding");

    let snapshot = registry.snapshot(TTL, 10);
    assert_eq!(snapshot.binding_count(), 2);
    assert_eq!(snapshot.creating_count(), 1);
    assert_eq!(snapshot.credential_counts().len(), 1);
    assert_eq!(snapshot.credential_counts()[0].bindings(), 2);
    assert_eq!(snapshot.bindings().len(), 2);
    for binding in snapshot.bindings() {
        assert_eq!(binding.session_hash_prefix().len(), 12);
        assert!(!binding.session_hash_prefix().contains("private"));
    }

    let aggregate = registry.snapshot(TTL, 0);
    assert_eq!(aggregate.binding_count(), 2);
    assert_eq!(aggregate.creating_count(), 1);
    assert!(aggregate.credential_counts().is_empty());
    assert!(aggregate.bindings().is_empty());

    assert_eq!(registry.clear_credential(credential_id.into()), 2);
    let snapshot = registry.snapshot(TTL, 10);
    assert_eq!(snapshot.binding_count(), 0);
    assert_eq!(snapshot.creating_count(), 1);
    assert_eq!(registry.clear_all(), 1);
}
