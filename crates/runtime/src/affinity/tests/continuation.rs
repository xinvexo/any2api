use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::{Duration, Instant},
};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect};
use any2api_protocol::api::MAX_BRIDGE_CONTINUATION_STATE_BYTES;

use super::{TTL, resolved_continuation_target, target};
use crate::affinity::{
    AffinityError, AffinityRegistry, BindingStart, ContinuationLookup,
    continuation::TEST_TOTAL_BYTES, hash::SessionHasher, registry::BindingState,
};

#[test]
fn continuation_identity_conflicts_are_rejected() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    registry
        .bind_ready_continuation(
            "resp-conflict",
            target(route_id, CredentialId::new()),
            None,
            TTL,
        )
        .expect("first continuation binding");

    assert_eq!(
        registry.bind_ready_continuation(
            "resp-conflict",
            target(route_id, CredentialId::new()),
            None,
            TTL,
        ),
        Err(AffinityError::IdentityConflict)
    );
}

#[test]
fn expired_continuation_identity_can_bind_to_a_new_target() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let first = target(route_id, CredentialId::new());
    let second = target(route_id, CredentialId::new());
    registry
        .bind_ready_continuation("resp-reused", first, None, TTL)
        .expect("first continuation binding");

    registry
        .bind_ready_continuation("resp-reused", second.clone(), None, Duration::ZERO)
        .expect("expired continuation identity can be reused");

    assert_eq!(
        resolved_continuation_target(&registry, "resp-reused", TTL),
        Some(second)
    );
}

#[test]
fn continuation_commit_defers_unrelated_expiration_to_the_sweeper() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let stale_identity = "resp-unrelated-stale";
    registry
        .bind_ready_continuation(stale_identity, target.clone(), None, TTL)
        .expect("stale continuation seed");
    let stale_key = registry.hasher.continuation(stale_identity);
    {
        let mut state = registry.state.lock().expect("affinity state");
        let BindingState::Bound { binding } = state
            .entries
            .get_mut(&stale_key)
            .expect("stale continuation entry")
        else {
            panic!("continuation must be bound");
        };
        binding.last_seen_at = Instant::now() - TTL - Duration::from_secs(1);
    }

    registry
        .bind_ready_continuation("resp-current", target, None, TTL)
        .expect("unrelated continuation commit");

    assert!(
        registry
            .state
            .lock()
            .expect("affinity state")
            .entries
            .contains_key(&stale_key),
        "ordinary commits must not scan the complete binding table"
    );
    assert_eq!(registry.sweep_expired(TTL), 1);
}

#[test]
fn elapsed_deadline_does_not_create_a_continuation_binding() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();

    assert_eq!(
        registry.bind_ready_continuation_before(
            "resp-too-late",
            target(route_id, CredentialId::new()),
            None,
            TTL,
            Instant::now() - Duration::from_millis(1),
        ),
        Err(AffinityError::DeadlineExceeded)
    );
    assert!(matches!(
        registry.resolve_continuation("resp-too-late", TTL, |_| true),
        ContinuationLookup::Missing
    ));
}

#[test]
fn pending_continuations_reserve_total_capacity_and_drop_releases_it() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let slots = TEST_TOTAL_BYTES / MAX_BRIDGE_CONTINUATION_STATE_BYTES;
    assert_eq!(slots, 4);

    let mut leases = (0..slots)
        .map(|index| {
            let raw = format!("resp-pending-{index}");
            let lease = registry
                .begin_pending_continuation(&raw, target(route_id, CredentialId::new()), TTL)
                .expect("pending reservation within total capacity");
            assert!(matches!(
                registry.resolve_continuation(&raw, TTL, |_| true),
                ContinuationLookup::Pending
            ));
            lease
        })
        .collect::<Vec<_>>();

    assert!(matches!(
        registry.begin_pending_continuation(
            "resp-over-capacity",
            target(route_id, CredentialId::new()),
            TTL,
        ),
        Err(AffinityError::ContinuationCapacity)
    ));

    drop(leases.pop());
    let replacement = registry
        .begin_pending_continuation(
            "resp-replacement",
            target(route_id, CredentialId::new()),
            TTL,
        )
        .expect("dropped reservation returns capacity");
    drop(replacement);
    drop(leases);
    assert_eq!(
        registry
            .state
            .lock()
            .expect("affinity state")
            .continuation_bytes,
        0
    );
}

#[tokio::test]
async fn dropping_pending_continuation_aborts_and_wakes_waiters() {
    let registry = AffinityRegistry::new();
    let mut changes = registry.subscribe_scheduler_epoch();
    let lease = registry
        .begin_pending_continuation(
            "resp-aborted",
            target(ModelRouteId::new(), CredentialId::new()),
            TTL,
        )
        .expect("pending continuation");

    drop(lease);
    changes.changed().await.expect("abort wakes waiters");
    assert!(matches!(
        registry.resolve_continuation("resp-aborted", TTL, |_| true),
        ContinuationLookup::Missing
    ));
}

#[test]
fn panic_unwind_drops_pending_continuation_and_returns_capacity() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let unwind_registry = registry.clone();
    let result = catch_unwind(AssertUnwindSafe(move || {
        let _lease = unwind_registry
            .begin_pending_continuation("resp-unwind", target(route_id, CredentialId::new()), TTL)
            .expect("pending continuation");
        panic!("fault injection after pending reservation");
    }));

    assert!(result.is_err());
    assert!(matches!(
        registry.resolve_continuation("resp-unwind", TTL, |_| true),
        ContinuationLookup::Missing
    ));
    assert_eq!(registry.continuation_bytes_for_test(), 0);
}

#[test]
fn session_and_continuation_hashes_use_separate_domains() {
    let hasher = SessionHasher::new();
    let route_id = ModelRouteId::new();
    let raw = "same-private-identifier";

    assert_ne!(
        hasher.continuation(raw),
        hasher.session(ProtocolDialect::OpenAiResponses, route_id, raw)
    );
    assert_ne!(
        hasher.session(ProtocolDialect::OpenAiResponses, route_id, raw),
        hasher.session(ProtocolDialect::AnthropicMessages, route_id, raw)
    );
    assert_ne!(
        hasher.session(ProtocolDialect::OpenAiResponses, route_id, raw),
        hasher.session(ProtocolDialect::OpenAiResponses, ModelRouteId::new(), raw)
    );
}

#[test]
fn a_new_registry_starts_without_session_or_continuation_bindings() {
    let route_id = ModelRouteId::new();
    let target = target(route_id, CredentialId::new());
    let registry = AffinityRegistry::new();
    let mut lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-before-restart",
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
        .bind_ready_continuation("resp-before-restart", target, None, TTL)
        .expect("continuation binding");

    let restarted = AffinityRegistry::new();
    assert!(matches!(
        restarted.resolve_continuation("resp-before-restart", TTL, |_| true),
        ContinuationLookup::Missing
    ));
    assert!(matches!(
        restarted
            .begin_session(
                ProtocolDialect::OpenAiResponses,
                route_id,
                "session-before-restart",
                TTL,
            )
            .expect("new registry creates the session binding"),
        BindingStart::Create(_)
    ));
}
