use std::{sync::Arc, time::Duration};

use super::{BlockingRefreshTransport, RefreshTestContext};

#[tokio::test]
async fn oauth_refresh_reuses_routing_health_but_isolates_authentication_health() {
    let transport = Arc::new(BlockingRefreshTransport::new());
    let context = RefreshTestContext::with_account(Arc::clone(&transport)).await;
    let id = context.account_id.expect("OAuth account");
    let initial = context.snapshots.load();
    let old_generation = Arc::clone(
        initial
            .routing_credentials()
            .iter()
            .find(|credential| credential.id() == id.into())
            .expect("OAuth routing projection")
            .binding()
            .generation(),
    );
    old_generation
        .health()
        .record_quota_exhaustion(Duration::from_secs(60), Some(10), Some(10));
    assert!(old_generation.health().record_authentication_failure());
    drop(initial);
    transport.release();

    assert_eq!(
        context
            .refresher
            .refresh_if_due(id, 1)
            .await
            .expect("refresh result"),
        Some(2)
    );

    let refreshed = context.snapshots.load();
    let account = refreshed
        .oauth_accounts()
        .get(id)
        .expect("refreshed account");
    let new_generation = Arc::clone(
        refreshed
            .routing_credentials()
            .iter()
            .find(|credential| credential.id() == id.into())
            .expect("refreshed OAuth routing projection")
            .binding()
            .generation(),
    );
    assert_eq!(account.account_generation(), 1);
    assert_eq!(new_generation.routing_generation(), 1);
    assert_eq!(new_generation.authentication_version(), 2);
    assert!(!Arc::ptr_eq(&old_generation, &new_generation));
    assert!(!new_generation.health().has_auth_error());
    let quota = new_generation
        .health()
        .quota_exhaustion()
        .expect("routing quota health survives Token refresh");
    assert_eq!(quota.used, Some(10));
    assert_eq!(quota.limit, Some(10));

    assert!(old_generation.health().clear_auth_error());
    assert!(old_generation.health().record_authentication_failure());
    assert!(!new_generation.health().has_auth_error());
}
