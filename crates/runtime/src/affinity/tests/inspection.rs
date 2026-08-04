use std::time::{Duration, Instant};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect};

use super::{TTL, resolved_continuation_target, target};
use crate::affinity::{AffinityRegistry, BindingStart, ContinuationLookup};

#[test]
fn expired_session_and_continuation_bindings_are_not_reused() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let mut lease = match registry
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
    lease.mark_attempting().expect("promote session binding");
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
        .bind_ready_continuation("resp-expired", target, None, TTL)
        .expect("continuation binding");
    assert!(matches!(
        registry.resolve_continuation("resp-expired", Duration::ZERO, |_| true),
        ContinuationLookup::Missing
    ));
}

#[test]
fn sweep_uses_one_ttl_for_session_and_continuation_bindings() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let mut lease = match registry
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
    lease.mark_attempting().expect("promote session binding");
    lease
        .commit(target.clone())
        .expect("commit session binding");
    registry
        .bind_ready_continuation("resp-sweep", target, None, TTL)
        .expect("continuation binding");

    assert_eq!(registry.sweep_expired(TTL), 0);
    assert_eq!(registry.sweep_expired(Duration::ZERO), 2);
    assert!(matches!(
        registry.resolve_continuation("resp-sweep", TTL, |_| true),
        ContinuationLookup::Missing
    ));
}

#[test]
fn session_and_continuation_access_refresh_the_same_ttl() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let mut lease = match registry
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
    lease.mark_attempting().expect("promote session binding");
    lease
        .commit(target.clone())
        .expect("commit session binding");
    registry
        .bind_ready_continuation("resp-refresh", target, None, TTL)
        .expect("continuation binding");

    let stale_at = Instant::now() - Duration::from_secs(60);
    registry.stale_bound_entries_for_test(stale_at);

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
    assert!(matches!(
        registry.resolve_continuation("resp-refresh", TTL, |_| true),
        ContinuationLookup::Ready(_)
    ));

    let last_seen = registry.bound_last_seen_for_test();
    assert_eq!(last_seen.len(), 2);
    assert!(last_seen.iter().all(|seen_at| *seen_at > stale_at));
}

#[test]
fn unavailable_continuation_target_does_not_refresh_ttl() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    registry
        .bind_ready_continuation("resp-unavailable", target.clone(), None, TTL)
        .expect("continuation binding");

    let stale_at = Instant::now() - Duration::from_secs(60);
    registry.stale_bound_entries_for_test(stale_at);

    assert!(matches!(
        registry.resolve_continuation("resp-unavailable", TTL, |resolved| resolved != &target),
        ContinuationLookup::Missing
    ));
    assert_eq!(
        registry.bound_last_seen_for_test(),
        vec![stale_at],
        "unavailable continuation remains until cleanup without a TTL refresh"
    );

    assert!(matches!(
        registry.resolve_continuation("resp-unavailable", TTL, |resolved| resolved == &target),
        ContinuationLookup::Ready(_)
    ));
    let last_seen = registry.bound_last_seen_for_test();
    assert_eq!(last_seen.len(), 1);
    assert!(last_seen[0] > stale_at);
}

#[test]
fn snapshots_only_count_explicit_sessions_honored_by_the_current_policy() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let credential_id = CredentialId::new();
    let target = target(route_id, credential_id);
    let mut lease = match registry
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
    lease.mark_attempting().expect("promote session binding");
    lease
        .commit(target.clone())
        .expect("commit session binding");
    registry
        .bind_ready_continuation("private-response-id", target.clone(), None, TTL)
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

    let enabled = registry.snapshot(TTL, true);
    assert_eq!(enabled.active_session_count(), 1);
    assert_eq!(enabled.creating_session_count(), 1);

    let disabled = registry.snapshot(TTL, false);
    assert_eq!(disabled.active_session_count(), 0);
    assert_eq!(disabled.creating_session_count(), 0);

    assert_eq!(
        resolved_continuation_target(&registry, "private-response-id", TTL),
        Some(target)
    );

    assert_eq!(registry.clear_credential(credential_id.into()), 2);
    let snapshot = registry.snapshot(TTL, true);
    assert_eq!(snapshot.active_session_count(), 0);
    assert_eq!(snapshot.creating_session_count(), 1);
    assert_eq!(registry.clear_all(), 1);
}
