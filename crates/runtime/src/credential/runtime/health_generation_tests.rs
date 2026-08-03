use std::{sync::Arc, time::Duration};

use any2api_domain::{CredentialId, RoutingCredentialId};
use any2api_provider::api::ProviderSecret;

use super::{CredentialAuthentication, CredentialGenerationDefinition, CredentialRuntimeHandle};
use crate::routing::SchedulerEpoch;

#[tokio::test]
async fn authentication_rotation_reuses_only_routing_health() {
    let epoch = SchedulerEpoch::new();
    let handle = CredentialRuntimeHandle::new(
        RoutingCredentialId::provider_credential(CredentialId::new()),
        definition(1, 1, "old-secret"),
        Arc::clone(&epoch),
    );
    let old = handle.current_binding(None);
    old.generation()
        .health()
        .record_quota_exhaustion(Duration::from_secs(60), Some(5), Some(5));
    assert!(old.generation().health().record_authentication_failure());

    let rotated_auth = handle.reconcile(None, definition(1, 2, "new-secret"));
    assert!(!Arc::ptr_eq(old.generation(), rotated_auth.generation()));
    assert!(!rotated_auth.generation().health().has_auth_error());
    assert!(
        rotated_auth
            .generation()
            .health()
            .quota_exhaustion()
            .is_some()
    );

    assert!(old.generation().health().clear_auth_error());
    assert!(old.generation().health().record_authentication_failure());
    assert!(!rotated_auth.generation().health().has_auth_error());

    assert!(rotated_auth.generation().health().clear_quota_exhaustion());
    assert!(old.generation().health().quota_exhaustion().is_none());
    rotated_auth
        .generation()
        .health()
        .record_quota_exhaustion(Duration::from_secs(60), None, None);
    assert!(
        rotated_auth
            .generation()
            .health()
            .record_authentication_failure()
    );

    let new_routing_identity = handle.reconcile(None, definition(2, 3, "other-secret"));
    assert!(!new_routing_identity.generation().health().has_auth_error());
    assert!(
        new_routing_identity
            .generation()
            .health()
            .quota_exhaustion()
            .is_none()
    );
}

fn definition(
    routing_generation: u64,
    authentication_version: u64,
    secret: &str,
) -> CredentialGenerationDefinition {
    CredentialGenerationDefinition::new(
        routing_generation,
        authentication_version,
        CredentialAuthentication::provider_api_key(Arc::new(ProviderSecret::new(secret))),
    )
}
