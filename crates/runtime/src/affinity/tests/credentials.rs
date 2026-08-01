use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect};

use super::{TTL, target};
use crate::affinity::{AffinityRegistry, BindingStart, ContinuationLookup};

#[test]
fn credential_cleanup_does_not_invalidate_an_in_progress_binding_commit() {
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

    assert_eq!(registry.clear_credential(credential_id.into()), 0);
    lease
        .commit(target.clone())
        .expect("captured request commit");
    registry
        .bind_ready_continuation("response-being-removed", target, None, TTL)
        .expect("captured continuation commit");
    let snapshot = registry.snapshot(TTL, true);
    assert_eq!(snapshot.active_session_count(), 1);
    assert_eq!(snapshot.creating_session_count(), 0);
}

#[test]
fn explicit_credential_cleanup_clears_existing_bindings() {
    let registry = AffinityRegistry::new();
    let route_id = ModelRouteId::new();
    let credential_id = CredentialId::new();
    let target = target(route_id, credential_id);
    registry
        .bind_ready_continuation("response-before-removal", target, None, TTL)
        .expect("active credential binding");

    assert_eq!(registry.clear_credential(credential_id.into()), 1);

    assert!(matches!(
        registry.resolve_continuation("response-before-removal", TTL, |_| true),
        ContinuationLookup::Missing
    ));
    assert_eq!(registry.snapshot(TTL, true).active_session_count(), 0);
}

#[tokio::test]
async fn credential_cleanup_aborts_pending_continuation_and_wakes_waiters() {
    let registry = AffinityRegistry::new();
    let mut changes = registry.subscribe_scheduler_epoch();
    let credential_id = CredentialId::new();
    let lease = registry
        .begin_pending_continuation(
            "response-being-created",
            target(ModelRouteId::new(), credential_id),
            TTL,
        )
        .expect("pending continuation");

    assert_eq!(registry.clear_credential(credential_id.into()), 1);
    changes
        .changed()
        .await
        .expect("credential cleanup wakes continuation waiters");
    assert!(matches!(
        registry.resolve_continuation("response-being-created", TTL, |_| true),
        ContinuationLookup::Missing
    ));
    assert_eq!(registry.continuation_bytes_for_test(), 0);
    drop(lease);
}
