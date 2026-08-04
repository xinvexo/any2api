use std::sync::Arc;

use any2api_domain::{
    RetrySafety, RoutingCredentialId, SettingsConfiguration, UpstreamErrorClassification,
    UpstreamErrorKind,
};

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
        ),
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
async fn quota_query_refreshes_once_after_authentication_rejection() {
    let context = QuotaTestContext::new(1, AuthenticationMode::RejectOnce).await;

    context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("quota query after refresh");

    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(
        context.transport.usage_authorizations(),
        ["Bearer old-access", "Bearer new-access"]
    );
    assert_eq!(
        context
            .snapshots
            .load()
            .oauth_accounts()
            .get(context.account_id)
            .expect("OAuth account")
            .token_version(),
        2
    );
}

#[tokio::test]
async fn a_second_quota_401_does_not_refresh_or_query_a_third_time() {
    let context = QuotaTestContext::new(1, AuthenticationMode::AlwaysReject).await;

    assert!(matches!(
        context.service.refresh_quota(context.account_id).await,
        Err(OAuthQuotaError::AuthenticationFailed)
    ));
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(context.transport.usage_authorizations().len(), 2);
}

#[tokio::test]
async fn a_failed_token_refresh_does_not_claim_the_account_is_invalid() {
    let context = QuotaTestContext::new(1, AuthenticationMode::RefreshRejected).await;

    assert!(matches!(
        context.service.refresh_quota(context.account_id).await,
        Err(OAuthQuotaError::AuthenticationRefreshFailed)
    ));
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(context.transport.usage_calls(), 1);
}

#[tokio::test]
async fn cloudflare_codex_403_is_classified_from_the_original_response() {
    let context = QuotaTestContext::new(1, AuthenticationMode::CodexCloudflareBlocked).await;

    assert!(matches!(
        context.service.refresh_quota(context.account_id).await,
        Err(OAuthQuotaError::ProviderEgressRestricted)
    ));

    let captured = context.transport.captured();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].path, "/backend-api/wham/usage");
    assert_eq!(
        captured[0].authorization.as_deref(),
        Some("Bearer old-access")
    );
    assert_eq!(captured[0].account_id.as_deref(), Some("account-123"));
}

#[tokio::test]
async fn unknown_codex_403_stays_neutral_without_a_secondary_request() {
    let context = QuotaTestContext::new(1, AuthenticationMode::CodexUnknownForbidden).await;

    assert!(matches!(
        context.service.refresh_quota(context.account_id).await,
        Err(OAuthQuotaError::UpstreamRejected(403))
    ));
    let captured = context.transport.captured();
    assert_eq!(captured.len(), 1);
    assert!(captured[0].authorization.is_some());
    assert!(captured[0].account_id.is_some());
}

#[tokio::test]
async fn invalid_grant_marks_the_account_authentication_as_failed() {
    let context = QuotaTestContext::new(1, AuthenticationMode::RefreshInvalidGrant).await;

    assert!(matches!(
        context.service.refresh_quota(context.account_id).await,
        Err(OAuthQuotaError::AuthenticationFailed)
    ));
    let snapshot = context.snapshots.load();
    let health = snapshot
        .credential_runtime(RoutingCredentialId::oauth_account(context.account_id))
        .expect("OAuth runtime")
        .generation()
        .health();
    assert_eq!(
        health.availability("gpt-5.5"),
        Err(HealthAcquireError::Permanent)
    );
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(context.transport.usage_calls(), 1);
}

#[tokio::test]
async fn reused_refresh_token_is_permanent_and_is_submitted_only_once() {
    let context = QuotaTestContext::new(1, AuthenticationMode::RefreshTokenReused).await;

    for _ in 0..2 {
        assert!(matches!(
            context.service.refresh_quota(context.account_id).await,
            Err(OAuthQuotaError::AuthenticationFailed)
        ));
    }

    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(context.transport.usage_calls(), 2);
}

#[tokio::test]
async fn rejected_access_token_without_refresh_token_is_authentication_failed() {
    let context = QuotaTestContext::new_without_refresh_token().await;

    assert!(matches!(
        context.service.refresh_quota(context.account_id).await,
        Err(OAuthQuotaError::AuthenticationFailed)
    ));
    let snapshot = context.snapshots.load();
    let health = snapshot
        .credential_runtime(RoutingCredentialId::oauth_account(context.account_id))
        .expect("OAuth runtime")
        .generation()
        .health();
    assert_eq!(
        health.availability("gpt-5.5"),
        Err(HealthAcquireError::Permanent)
    );
    assert_eq!(context.transport.refresh_calls(), 0);
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
