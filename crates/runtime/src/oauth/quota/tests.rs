use std::sync::Arc;

use any2api_domain::{
    RetrySafety, RoutingCredentialId, SettingsConfiguration, UpstreamErrorClassification,
    UpstreamErrorKind, UpstreamFailureAttribution,
};
use any2api_transport::api::TransportTrafficClass;

use super::{
    OAuthQuotaError,
    test_support::{AuthenticationMode, QuotaTestContext},
};
use crate::health::{HealthAcquireError, ReliabilityPolicy};
use uuid::Uuid;

#[tokio::test]
async fn query_and_reset_use_direct_transport_and_clear_temporary_cooldowns() {
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
    changes.changed().await.expect("quota deletion change");
    assert_eq!(*changes.borrow_and_update(), 2);
    assert_eq!(
        context
            .service
            .cached_quota(context.account_id)
            .await
            .expect("cleared cached quota"),
        None
    );
    assert_eq!(generation.health().availability("gpt-5.5"), Ok(()));
    assert!(context.runtime.scheduler_epoch() > epoch_before);

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
        ]
    );
    assert!(captured.iter().all(|request| {
        request.proxy_id == any2api_domain::ProxyProfileId::DIRECT
            && request.account_id.as_deref() == Some("account-123")
            && request.strict_ssrf == context.snapshots.load().settings().upstream().strict_ssrf()
            && request.traffic_class == TransportTrafficClass::OAuthQuota
    }));
    let redeem_id = serde_json::from_slice::<serde_json::Value>(
        &captured.last().expect("consume request").body,
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
