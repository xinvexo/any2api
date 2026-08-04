use std::time::{Duration, Instant};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect};

use super::{TTL, target};
use crate::affinity::{AffinityError, AffinityRegistry, BindingCreationPhase, BindingStart};

#[tokio::test]
async fn session_creation_is_single_flight_and_commit_wakes_waiters() {
    let registry = AffinityRegistry::new();
    let mut changes = registry.subscribe_scheduler_epoch();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let mut lease = match registry
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
        BindingStart::Wait(BindingCreationPhase::Selecting) => {}
        other => panic!("concurrent caller must wait: {other:?}"),
    }

    lease.mark_attempting().expect("promote creating lease");
    changes.changed().await.expect("promotion wakes the waiter");
    match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-one",
            TTL,
        )
        .expect("attempting session binding")
    {
        BindingStart::Wait(BindingCreationPhase::Attempting) => {}
        other => panic!("concurrent caller must observe the active attempt: {other:?}"),
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
        BindingStart::Wait(BindingCreationPhase::Selecting) => {}
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
async fn releasing_selecting_for_queue_wait_removes_it_and_returns_the_wake_epoch() {
    let registry = AffinityRegistry::new();
    let mut changes = registry.subscribe_scheduler_epoch();
    let route_id = ModelRouteId::new();
    let lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-queue-handoff",
            TTL,
        )
        .expect("session lease")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };

    let released_epoch = lease.release_for_wait().expect("release selecting lease");
    changes
        .changed()
        .await
        .expect("release wakes queue waiters");
    assert_eq!(*changes.borrow_and_update(), released_epoch);
    assert!(matches!(
        registry
            .begin_session(
                ProtocolDialect::OpenAiResponses,
                route_id,
                "session-queue-handoff",
                TTL,
            )
            .expect("session selection can be reacquired"),
        BindingStart::Create(_)
    ));
}

#[test]
fn selecting_lease_cannot_commit_without_attempt_promotion() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-unpromoted",
            TTL,
        )
        .expect("session lease")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };

    assert_eq!(
        lease.commit(target(route_id, CredentialId::new())),
        Err(AffinityError::LeaseLost)
    );
    assert!(matches!(
        registry
            .begin_session(
                ProtocolDialect::OpenAiResponses,
                route_id,
                "session-unpromoted",
                TTL,
            )
            .expect("failed commit releases the lease"),
        BindingStart::Create(_)
    ));
}

#[tokio::test]
async fn active_creating_state_is_not_expired_by_binding_cleanup() {
    let registry = AffinityRegistry::new();
    let mut changes = registry.subscribe_scheduler_epoch();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let mut lease = match registry
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

    lease.mark_attempting().expect("promote active creator");
    changes.changed().await.expect("promotion wakes waiters");
    assert_eq!(registry.sweep_expired(Duration::ZERO), 0);
    assert_eq!(
        registry
            .snapshot(Duration::ZERO, true)
            .creating_session_count(),
        1
    );
    match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-active-creator",
            TTL,
        )
        .expect("concurrent session")
    {
        BindingStart::Wait(BindingCreationPhase::Attempting) => {}
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
