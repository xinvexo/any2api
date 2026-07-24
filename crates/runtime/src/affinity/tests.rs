use std::time::{Duration, Instant};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect, RouteTargetId};

use super::{AffinityError, AffinityRegistry, AffinityTarget, SoftBindingStart};

const SOFT_TTL: Duration = Duration::from_secs(60);
const HARD_TTL: Duration = Duration::from_secs(120);
const CREATING_TTL: Duration = Duration::from_secs(5);

#[tokio::test]
async fn soft_binding_creation_is_single_flight_and_commit_wakes_waiters() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let lease = match registry
        .begin_soft(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-one",
            SOFT_TTL,
            CREATING_TTL,
        )
        .expect("first soft binding")
    {
        SoftBindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    let mut waiter = match registry
        .begin_soft(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-one",
            SOFT_TTL,
            CREATING_TTL,
        )
        .expect("concurrent soft binding")
    {
        SoftBindingStart::Wait(waiter) => waiter,
        other => panic!("concurrent caller must wait: {other:?}"),
    };

    lease.commit(target.clone()).expect("commit soft binding");
    waiter.changed().await.expect("commit wakes the waiter");

    let binding = match registry
        .begin_soft(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-one",
            SOFT_TTL,
            CREATING_TTL,
        )
        .expect("bound soft session")
    {
        SoftBindingStart::Bound(binding) => binding,
        other => panic!("session must now be bound: {other:?}"),
    };
    assert_eq!(binding.target(), &target);
}

#[tokio::test]
async fn dropping_a_soft_lease_wakes_waiters_and_allows_recreation() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let lease = match registry
        .begin_soft(
            ProtocolDialect::AnthropicMessages,
            route_id,
            "session-drop",
            SOFT_TTL,
            CREATING_TTL,
        )
        .expect("first soft binding")
    {
        SoftBindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    let mut waiter = match registry
        .begin_soft(
            ProtocolDialect::AnthropicMessages,
            route_id,
            "session-drop",
            SOFT_TTL,
            CREATING_TTL,
        )
        .expect("concurrent soft binding")
    {
        SoftBindingStart::Wait(waiter) => waiter,
        other => panic!("concurrent caller must wait: {other:?}"),
    };

    drop(lease);
    waiter.changed().await.expect("drop wakes the waiter");
    assert!(matches!(
        registry
            .begin_soft(
                ProtocolDialect::AnthropicMessages,
                route_id,
                "session-drop",
                SOFT_TTL,
                CREATING_TTL,
            )
            .expect("recreated soft binding"),
        SoftBindingStart::Create(_)
    ));
}

#[test]
fn expired_bindings_are_not_reused() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let lease = match registry
        .begin_soft(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "soft-expired",
            SOFT_TTL,
            CREATING_TTL,
        )
        .expect("soft lease")
    {
        SoftBindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    lease.commit(target.clone()).expect("commit soft binding");
    assert!(matches!(
        registry
            .begin_soft(
                ProtocolDialect::OpenAiResponses,
                route_id,
                "soft-expired",
                Duration::ZERO,
                CREATING_TTL,
            )
            .expect("expired binding is replaced"),
        SoftBindingStart::Create(_)
    ));

    registry
        .bind_hard("resp-expired", target, HARD_TTL)
        .expect("hard binding");
    assert!(
        registry
            .resolve_hard("resp-expired", Duration::ZERO)
            .is_none()
    );
}

#[test]
fn hard_identity_conflicts_are_rejected() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    registry
        .bind_hard(
            "resp-conflict",
            target(route_id, CredentialId::new()),
            HARD_TTL,
        )
        .expect("first hard binding");

    assert_eq!(
        registry.bind_hard(
            "resp-conflict",
            target(route_id, CredentialId::new()),
            HARD_TTL,
        ),
        Err(AffinityError::IdentityConflict)
    );
}

#[test]
fn elapsed_deadline_does_not_create_a_hard_binding() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();

    assert_eq!(
        registry.bind_hard_before(
            "resp-too-late",
            target(route_id, CredentialId::new()),
            HARD_TTL,
            Instant::now() - Duration::from_millis(1),
        ),
        Err(AffinityError::DeadlineExceeded)
    );
    assert!(registry.resolve_hard("resp-too-late", HARD_TTL).is_none());
}

#[test]
fn elapsed_deadline_does_not_commit_a_soft_binding() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let lease = match registry
        .begin_soft(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "soft-too-late",
            SOFT_TTL,
            CREATING_TTL,
        )
        .expect("soft binding lease")
    {
        SoftBindingStart::Create(lease) => lease,
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
            .begin_soft(
                ProtocolDialect::OpenAiResponses,
                route_id,
                "soft-too-late",
                SOFT_TTL,
                CREATING_TTL,
            )
            .expect("expired lease was removed"),
        SoftBindingStart::Create(_)
    ));
}

#[test]
fn a_new_process_registry_does_not_restore_hard_bindings() {
    let route_id = ModelRouteId::new();
    let registry = AffinityRegistry::new();
    registry
        .bind_hard(
            "resp-before-restart",
            target(route_id, CredentialId::new()),
            HARD_TTL,
        )
        .expect("hard binding");
    assert!(
        registry
            .resolve_hard("resp-before-restart", HARD_TTL)
            .is_some()
    );

    let restarted = AffinityRegistry::new();
    assert!(
        restarted
            .resolve_hard("resp-before-restart", HARD_TTL)
            .is_none()
    );
}

#[test]
fn snapshots_are_redacted_and_cleanup_is_scoped() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let credential_id = CredentialId::new();
    let target = target(route_id, credential_id);
    let lease = match registry
        .begin_soft(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "private-session-id",
            SOFT_TTL,
            CREATING_TTL,
        )
        .expect("soft binding")
    {
        SoftBindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };
    lease.commit(target.clone()).expect("commit soft binding");
    registry
        .bind_hard("private-response-id", target, HARD_TTL)
        .expect("hard binding");
    let creating_route = ModelRouteId::new();
    let _creating = registry
        .begin_soft(
            ProtocolDialect::AnthropicMessages,
            creating_route,
            "creating-session",
            SOFT_TTL,
            CREATING_TTL,
        )
        .expect("creating binding");

    let snapshot = registry.snapshot(SOFT_TTL, HARD_TTL, CREATING_TTL, 10);
    assert_eq!(snapshot.soft_binding_count(), 1);
    assert_eq!(snapshot.hard_binding_count(), 1);
    assert_eq!(snapshot.creating_count(), 1);
    assert_eq!(snapshot.credential_counts().len(), 1);
    assert_eq!(snapshot.credential_counts()[0].soft_bindings(), 1);
    assert_eq!(snapshot.credential_counts()[0].hard_bindings(), 1);
    assert_eq!(snapshot.bindings().len(), 2);
    for binding in snapshot.bindings() {
        assert_eq!(binding.session_hash_prefix().len(), 12);
        assert!(!binding.session_hash_prefix().contains("private"));
    }

    assert_eq!(registry.clear_credential(credential_id.into()), 2);
    let snapshot = registry.snapshot(SOFT_TTL, HARD_TTL, CREATING_TTL, 10);
    assert_eq!(snapshot.soft_binding_count(), 0);
    assert_eq!(snapshot.hard_binding_count(), 0);
    assert_eq!(snapshot.creating_count(), 1);
    assert_eq!(registry.clear_all(), 1);
}

fn target(route_id: ModelRouteId, credential_id: CredentialId) -> AffinityTarget {
    AffinityTarget::new(
        route_id,
        RouteTargetId::new(),
        credential_id.into(),
        "upstream-model",
        ProtocolDialect::OpenAiResponses,
    )
}
