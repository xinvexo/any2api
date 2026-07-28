use std::time::{Duration, Instant};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect};

use super::{TTL, target};
use crate::affinity::{AffinityError, AffinityRegistry, BindingStart, hash::SessionHasher};

#[test]
fn continuation_identity_conflicts_are_rejected() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    registry
        .bind_continuation("resp-conflict", target(route_id, CredentialId::new()), TTL)
        .expect("first continuation binding");

    assert_eq!(
        registry.bind_continuation("resp-conflict", target(route_id, CredentialId::new()), TTL,),
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
        .bind_continuation("resp-reused", first, TTL)
        .expect("first continuation binding");

    registry
        .bind_continuation("resp-reused", second.clone(), Duration::ZERO)
        .expect("expired continuation identity can be reused");

    assert_eq!(
        registry.resolve_continuation("resp-reused", TTL),
        Some(second)
    );
}

#[test]
fn elapsed_deadline_does_not_create_a_continuation_binding() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();

    assert_eq!(
        registry.bind_continuation_before(
            "resp-too-late",
            target(route_id, CredentialId::new()),
            TTL,
            Instant::now() - Duration::from_millis(1),
        ),
        Err(AffinityError::DeadlineExceeded)
    );
    assert!(
        registry
            .resolve_continuation("resp-too-late", TTL)
            .is_none()
    );
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
    let lease = match registry
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
    lease
        .commit(target.clone())
        .expect("commit session binding");
    registry
        .bind_continuation("resp-before-restart", target, TTL)
        .expect("continuation binding");

    let restarted = AffinityRegistry::new();
    assert!(
        restarted
            .resolve_continuation("resp-before-restart", TTL)
            .is_none()
    );
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
