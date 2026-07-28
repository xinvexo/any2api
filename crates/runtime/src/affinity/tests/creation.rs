use std::time::{Duration, Instant};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect};

use super::{TTL, target};
use crate::affinity::{AffinityError, AffinityRegistry, BindingStart};

#[tokio::test]
async fn session_creation_is_single_flight_and_commit_wakes_waiters() {
    let registry = AffinityRegistry::new();
    let mut changes = registry.subscribe_scheduler_epoch();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-one",
            TTL,
        )
        .expect("first session binding")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-one",
            TTL,
        )
        .expect("concurrent session binding")
    {
        BindingStart::Wait => {}
        other => panic!("concurrent caller must wait: {other:?}"),
    }

    lease
        .commit(target.clone())
        .expect("commit session binding");
    changes.changed().await.expect("commit wakes the waiter");

    let binding = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-one",
            TTL,
        )
        .expect("bound session")
    {
        BindingStart::Bound(binding) => binding,
        other => panic!("session must now be bound: {other:?}"),
    };
    assert_eq!(binding.target(), &target);
}

#[tokio::test]
async fn dropping_a_session_lease_wakes_waiters_and_allows_recreation() {
    let registry = AffinityRegistry::new();
    let mut changes = registry.subscribe_scheduler_epoch();
    let route_id = ModelRouteId::new();
    let lease = match registry
        .begin_session(
            ProtocolDialect::AnthropicMessages,
            route_id,
            "session-drop",
            TTL,
        )
        .expect("first session binding")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    match registry
        .begin_session(
            ProtocolDialect::AnthropicMessages,
            route_id,
            "session-drop",
            TTL,
        )
        .expect("concurrent session binding")
    {
        BindingStart::Wait => {}
        other => panic!("concurrent caller must wait: {other:?}"),
    }

    drop(lease);
    changes.changed().await.expect("drop wakes the waiter");
    assert!(matches!(
        registry
            .begin_session(
                ProtocolDialect::AnthropicMessages,
                route_id,
                "session-drop",
                TTL,
            )
            .expect("recreated session binding"),
        BindingStart::Create(_)
    ));
}

#[tokio::test]
async fn active_creating_state_is_not_expired_by_binding_cleanup() {
    let registry = AffinityRegistry::new();
    let mut changes = registry.subscribe_scheduler_epoch();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-active-creator",
            TTL,
        )
        .expect("first session lease")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };

    assert_eq!(registry.sweep_expired(Duration::ZERO), 0);
    assert_eq!(registry.snapshot(Duration::ZERO, 10).creating_count(), 1);
    match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-active-creator",
            TTL,
        )
        .expect("concurrent session")
    {
        BindingStart::Wait => {}
        other => panic!("active creator must remain current: {other:?}"),
    }

    lease.commit(target).expect("commit active creator");
    changes.changed().await.expect("commit wakes waiter");
}

#[test]
fn elapsed_deadline_does_not_commit_a_session_binding() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-too-late",
            TTL,
        )
        .expect("session binding lease")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };

    assert_eq!(
        lease.commit_before(
            target(route_id, CredentialId::new()),
            Instant::now() - Duration::from_millis(1),
        ),
        Err(AffinityError::DeadlineExceeded)
    );
    assert!(matches!(
        registry
            .begin_session(
                ProtocolDialect::OpenAiResponses,
                route_id,
                "session-too-late",
                TTL,
            )
            .expect("expired lease was removed"),
        BindingStart::Create(_)
    ));
}
