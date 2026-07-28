use std::collections::HashSet;

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect};

use super::{TTL, target};
use crate::affinity::{AffinityError, AffinityRegistry, BindingStart};

#[test]
fn credential_removal_before_commit_prevents_a_binding_from_reappearing() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let credential_id = CredentialId::new();
    let target = target(route_id, credential_id);
    let lease = match registry
        .begin_session(
            ProtocolDialect::OpenAiResponses,
            route_id,
            "session-being-removed",
            TTL,
        )
        .expect("session lease")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("first caller must create the binding: {other:?}"),
    };

    registry.retain_credentials(&HashSet::new());
    assert_eq!(
        lease.commit(target.clone()),
        Err(AffinityError::TargetInactive)
    );
    assert_eq!(
        registry.bind_continuation("response-being-removed", target, TTL),
        Err(AffinityError::TargetInactive)
    );
    let snapshot = registry.snapshot(TTL, 10);
    assert_eq!(snapshot.binding_count(), 0);
    assert_eq!(snapshot.creating_count(), 0);
}

#[test]
fn credential_removal_after_commit_clears_the_binding() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let credential_id = CredentialId::new();
    let routing_id = credential_id.into();
    let target = target(route_id, credential_id);
    registry.retain_credentials(&HashSet::from([routing_id]));
    registry
        .bind_continuation("response-before-removal", target, TTL)
        .expect("active credential binding");

    registry.retain_credentials(&HashSet::new());

    assert!(
        registry
            .resolve_continuation("response-before-removal", TTL)
            .is_none()
    );
    assert_eq!(registry.snapshot(TTL, 10).binding_count(), 0);
}
