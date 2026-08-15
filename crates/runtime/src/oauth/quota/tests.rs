use std::sync::Arc;

use any2api_domain::{
    RetrySafety, RoutingCredentialId, SettingsConfiguration, UpstreamErrorClassification,
    UpstreamErrorKind, UpstreamFailureAttribution,
};
use any2api_storage::api::{OAuthModelCatalogSnapshotRepository, OAuthQuotaSnapshotRepository};
use any2api_transport::api::TransportTrafficClass;

use super::{
    OAuthQuotaError,
    test_support::{AuthenticationMode, QuotaTestContext},
};
use crate::health::{HealthAcquireError, ReliabilityPolicy};
use uuid::Uuid;

#[tokio::test]
async fn only_manual_quota_refresh_reads_and_persists_the_live_model_catalog() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;

    context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("quota only refresh");
    assert_eq!(context.transport.model_catalog_calls(), 0);

    context
        .service
        .refresh_quota_manually(context.account_id)
        .await
        .expect("manual quota refresh");
    assert_eq!(context.transport.model_catalog_calls(), 1);
    let catalogs = context
        .storage
        .load_oauth_model_catalog_snapshots()
        .await
        .expect("catalog snapshots");
    assert_eq!(catalogs.len(), 1);
    assert_eq!(
        catalogs[0].provider_kind,
        any2api_domain::ProviderKind::Codex
    );
    assert_eq!(catalogs[0].directory_scope, "free");
    assert_eq!(catalogs[0].models, ["gpt-catalog-a"]);
}

#[tokio::test]
async fn manual_quota_refresh_persists_a_catalog_larger_than_the_quota_limit() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;
    context.transport.set_large_model_catalog();

    let result = context
        .service
        .refresh_quota_manually(context.account_id)
        .await
        .expect("manual quota refresh");
    let (_, model_catalog_refreshed) = result.into_parts();

    assert!(model_catalog_refreshed);
    let catalogs = context
        .storage
        .load_oauth_model_catalog_snapshots()
        .await
        .expect("catalog snapshots");
    assert_eq!(catalogs.len(), 1);
    assert_eq!(catalogs[0].models, ["gpt-catalog-a"]);
}

#[tokio::test]
async fn manual_batch_refreshes_one_catalog_per_shared_scope() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;
    let second = context.add_codex_account("account-456").await;

    let result = context
        .service
        .refresh_quota_batch(vec![context.account_id, second])
        .await;

    let mut expected = vec![context.account_id, second];
    expected.sort();
    assert_eq!(result.succeeded(), expected);
    assert!(result.failed().is_empty());
    assert_eq!(result.model_catalog_refreshed_scopes(), 1);
    assert_eq!(result.model_catalog_failed_scopes(), 0);
    assert_eq!(context.transport.usage_calls(), 2);
    assert_eq!(context.transport.model_catalog_calls(), 1);
}

#[tokio::test]
async fn manual_batch_refreshes_twenty_codex_accounts_with_two_plan_catalog_queries() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;
    let mut ids = vec![context.account_id];
    for index in 1..10 {
        ids.push(
            context
                .add_codex_account(&format!("free-account-{index}"))
                .await,
        );
    }
    for index in 0..10 {
        ids.push(
            context
                .add_codex_account_with_plan(&format!("plus-account-{index}"), "plus")
                .await,
        );
    }

    let result = context.service.refresh_quota_batch(ids).await;

    assert_eq!(result.succeeded().len(), 20);
    assert!(result.failed().is_empty());
    assert_eq!(result.model_catalog_refreshed_scopes(), 2);
    assert_eq!(result.model_catalog_failed_scopes(), 0);
    assert_eq!(context.transport.usage_calls(), 20);
    assert_eq!(context.transport.model_catalog_calls(), 2);
    let catalogs = context
        .storage
        .load_oauth_model_catalog_snapshots()
        .await
        .expect("catalog snapshots");
    assert_eq!(catalogs.len(), 2);
    assert_eq!(
        catalogs
            .iter()
            .map(|catalog| catalog.directory_scope.as_str())
            .collect::<Vec<_>>(),
        ["free", "plus_or_pro"]
    );
}

#[tokio::test]
async fn reset_preserves_statistics_until_the_next_official_window() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;
    let mut changes = context.service.subscribe_quota_changes();
    let quota = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("quota query");
    assert_eq!(
        quota
            .usage
            .rate_limit
            .as_ref()
            .and_then(|limit| limit.windows.first())
            .map(|window| window.used_percent),
        Some(25.0)
    );
    assert_eq!(
        quota
            .usage
            .reset_credits
            .as_ref()
            .map(|credits| credits.available_count),
        Some(1)
    );
    changes.changed().await.expect("persisted quota change");
    assert_eq!(*changes.borrow_and_update(), 1);
    assert_eq!(
        context
            .service
            .cached_quota(context.account_id)
            .await
            .expect("cached quota"),
        Some(quota.clone())
    );
    assert_eq!(context.transport.usage_calls(), 1);
    let mut stored = context
        .storage
        .load_oauth_quota_snapshot(context.account_id)
        .await
        .expect("stored quota snapshot")
        .expect("persisted quota snapshot");
    let mut payload: serde_json::Value =
        serde_json::from_slice(&stored.payload).expect("quota snapshot payload");
    payload["estimator_state"]["windows"][0]["total_delta_used_percent"] = serde_json::json!(10.0);
    payload["estimator_state"]["windows"][0]["total_local_cost_credits"] = serde_json::json!(150.0);
    payload["estimator_state"]["windows"][0]["completed_interval_count"] = serde_json::json!(1);
    stored.payload = serde_json::to_vec(&payload).expect("seed cumulative statistics");
    context
        .storage
        .upsert_oauth_quota_snapshot(&stored)
        .await
        .expect("persist cumulative statistics");
    let accumulated = context
        .service
        .cached_quota(context.account_id)
        .await
        .expect("cached quota with cumulative statistics")
        .expect("persisted quota with cumulative statistics");
    assert_eq!(
        accumulated.estimates[0].estimated_capacity_credits,
        Some(1_500.0)
    );
    assert_eq!(accumulated.estimates[0].completed_interval_count, 1);

    let snapshot = context.snapshots.load();
    let generation = Arc::clone(
        snapshot
            .credential_runtime(RoutingCredentialId::oauth_account(context.account_id))
            .expect("OAuth runtime")
            .generation(),
    );
    generation.health().record(
        "gpt-5.5",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::QuotaExhausted,
            RetrySafety::RejectedBeforeExecution,
            None,
        )
        .with_attribution(UpstreamFailureAttribution::Credential),
        &ReliabilityPolicy::from_settings(SettingsConfiguration::defaults().reliability()),
    );
    assert!(matches!(
        generation.health().availability("gpt-5.5"),
        Err(HealthAcquireError::Temporary(_))
    ));
    let epoch_before = context.runtime.scheduler_epoch();
    drop(snapshot);

    let redeem_request_id =
        Uuid::parse_str("11111111-1111-4111-8111-111111111111").expect("reset request UUID");
    let reset = context
        .service
        .reset_quota(context.account_id, redeem_request_id)
        .await
        .expect("quota reset");
    assert_eq!(reset.windows_reset, 2);
    changes.changed().await.expect("pre-reset quota change");
    assert_eq!(*changes.borrow_and_update(), 2);
    let before_new_window = context
        .service
        .cached_quota(context.account_id)
        .await
        .expect("cached quota after reset")
        .expect("persisted pre-reset observation");
    assert_eq!(before_new_window.estimates, accumulated.estimates);
    assert_eq!(
        before_new_window.estimates[0].estimated_capacity_credits,
        Some(1_500.0)
    );
    assert_eq!(generation.health().availability("gpt-5.5"), Ok(()));
    assert!(context.runtime.scheduler_epoch() > epoch_before);

    let next_window = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("post-reset quota refresh");
    changes.changed().await.expect("new quota window change");
    assert_eq!(*changes.borrow_and_update(), 3);
    assert_eq!(next_window.estimates[0].completed_interval_count, 1);
    assert_eq!(
        next_window.estimates[0].estimated_capacity_credits,
        Some(1_500.0)
    );

    let captured = context.transport.captured();
    assert_eq!(
        captured
            .iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        [
            "/backend-api/wham/usage",
            "/backend-api/wham/rate-limit-reset-credits",
            "/backend-api/wham/usage",
            "/backend-api/wham/rate-limit-reset-credits",
            "/backend-api/wham/rate-limit-reset-credits/consume",
            "/backend-api/wham/usage",
            "/backend-api/wham/rate-limit-reset-credits",
        ]
    );
    assert!(captured.iter().all(|request| {
        request.proxy_id == any2api_domain::ProxyProfileId::DIRECT
            && request.account_id.as_deref() == Some("account-123")
            && request.strict_ssrf == context.snapshots.load().settings().upstream().strict_ssrf()
            && request.traffic_class == TransportTrafficClass::OAuthQuota
    }));
    let redeem_id = serde_json::from_slice::<serde_json::Value>(
        &captured
            .iter()
            .find(|request| request.path == "/backend-api/wham/rate-limit-reset-credits/consume")
            .expect("consume request")
            .body,
    )
    .expect("consume body")["redeem_request_id"]
        .as_str()
        .expect("redeem request id")
        .to_owned();
    assert_eq!(redeem_id, redeem_request_id.to_string());
}

#[tokio::test]
async fn reset_without_available_credit_never_calls_consume() {
    let context = QuotaTestContext::new(0, AuthenticationMode::Accepted).await;

    assert!(matches!(
        context
            .service
            .reset_quota(context.account_id, Uuid::new_v4())
            .await,
        Err(OAuthQuotaError::NoResetCredits)
    ));
    assert_eq!(context.transport.consume_calls(), 0);
}

#[tokio::test]
async fn explicit_quota_exhaustion_blocks_routing_until_a_fresh_available_snapshot() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;
    context.transport.set_codex_exhausted(true);

    let exhausted = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("exhausted quota snapshot");
    assert!(
        exhausted
            .usage
            .account_status
            .as_ref()
            .and_then(|status| status.quota_exhaustion)
            .is_some()
    );
    let snapshot = context.snapshots.load();
    let generation = Arc::clone(
        snapshot
            .credential_runtime(RoutingCredentialId::oauth_account(context.account_id))
            .expect("OAuth runtime")
            .generation(),
    );
    assert!(matches!(
        generation.health().availability("gpt-5.5"),
        Err(HealthAcquireError::Temporary(_))
    ));
    generation.health().clear_temporary_cooldowns();
    assert_eq!(generation.health().availability("gpt-5.5"), Ok(()));
    let cached = context
        .service
        .cached_quota(context.account_id)
        .await
        .expect("cached exhausted snapshot")
        .expect("persisted exhausted snapshot");
    assert_eq!(cached, exhausted);
    assert_eq!(generation.health().availability("gpt-5.5"), Ok(()));
    drop(snapshot);

    context.transport.set_codex_exhausted(false);
    let available = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("available quota snapshot");
    assert!(available.usage.account_status.is_none());
    assert_eq!(generation.health().availability("gpt-5.5"), Ok(()));
}

#[tokio::test]
async fn real_credits_keep_an_exhausted_rolling_window_routable() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;
    context.transport.set_codex_exhausted(true);
    context.transport.set_codex_has_credits(true);

    let quota = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("quota with purchased Credits");

    assert_eq!(
        quota
            .usage
            .credits
            .as_ref()
            .and_then(|credits| credits.balance.as_deref()),
        Some("17.50")
    );
    assert!(quota.usage.account_status.is_none());
    let snapshot = context.snapshots.load();
    let generation = snapshot
        .credential_runtime(RoutingCredentialId::oauth_account(context.account_id))
        .expect("OAuth runtime")
        .generation();
    assert_eq!(generation.health().availability("gpt-5.5"), Ok(()));
}

#[tokio::test]
async fn concurrent_quota_refreshes_share_one_provider_query_and_result() {
    let context = QuotaTestContext::new_blocking_refresh(1).await;
    let first_service = Arc::clone(&context.service);
    let id = context.account_id;
    let first = tokio::spawn(async move { first_service.refresh_quota(id).await });
    context.transport.wait_for_usage().await;

    let second_service = Arc::clone(&context.service);
    let second = tokio::spawn(async move { second_service.refresh_quota(id).await });
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(
        context
            .transport
            .captured()
            .iter()
            .filter(|request| request.path == "/backend-api/wham/usage")
            .count(),
        1
    );
    context.transport.release_usage();

    let first = first.await.expect("first refresh").expect("quota result");
    let second = second.await.expect("second refresh").expect("quota result");
    assert_eq!(first, second);
    assert_eq!(context.transport.usage_calls(), 1);
}

#[tokio::test]
async fn reset_orders_older_and_waiting_refreshes_without_returning_a_stale_snapshot() {
    let context = QuotaTestContext::new_blocking_refresh(1).await;
    let refresh_service = Arc::clone(&context.service);
    let id = context.account_id;
    let refresh = tokio::spawn(async move { refresh_service.refresh_quota(id).await });
    context.transport.wait_for_usage().await;

    let reset_service = Arc::clone(&context.service);
    let mut reset = Box::pin(reset_service.reset_quota(id, Uuid::new_v4()));
    assert!(futures_util::poll!(reset.as_mut()).is_pending());
    let waiting_refresh_service = Arc::clone(&context.service);
    let mut waiting_refresh = Box::pin(waiting_refresh_service.refresh_quota(id));
    assert!(futures_util::poll!(waiting_refresh.as_mut()).is_pending());
    context.transport.release_usage();

    refresh.await.expect("refresh task").expect("quota refresh");
    reset.await.expect("quota reset");
    let refreshed = waiting_refresh.await.expect("post-reset quota refresh");
    assert_eq!(
        context
            .service
            .cached_quota(context.account_id)
            .await
            .expect("cached quota after reset"),
        Some(refreshed)
    );
    assert_eq!(context.transport.usage_calls(), 3);
    assert_eq!(context.transport.consume_calls(), 1);
}

#[tokio::test]
async fn concurrent_resets_serialize_and_only_consume_the_last_credit_once() {
    let context = QuotaTestContext::new_blocking_reset(1).await;
    let first_service = Arc::clone(&context.service);
    let id = context.account_id;
    let first = tokio::spawn(async move { first_service.reset_quota(id, Uuid::new_v4()).await });
    context.transport.wait_for_consume().await;

    let second_service = Arc::clone(&context.service);
    let second = tokio::spawn(async move { second_service.reset_quota(id, Uuid::new_v4()).await });
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(context.transport.usage_calls(), 1);
    context.transport.release_consume();

    assert_eq!(
        first
            .await
            .expect("first reset")
            .expect("reset result")
            .windows_reset,
        2
    );
    assert!(matches!(
        second.await.expect("second reset"),
        Err(OAuthQuotaError::NoResetCredits)
    ));
    assert_eq!(context.transport.consume_calls(), 1);
    assert_eq!(context.transport.usage_calls(), 2);
}
