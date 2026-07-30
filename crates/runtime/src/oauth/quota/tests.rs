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

#[tokio::test]
async fn query_and_reset_use_direct_transport_and_clear_temporary_cooldowns() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;
    let quota = context
        .service
        .query_quota(context.account_id)
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

    let reset = context
        .service
        .reset_quota(context.account_id)
        .await
        .expect("quota reset");
    assert_eq!(reset.windows_reset, 2);
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
    assert!(uuid::Uuid::parse_str(&redeem_id).is_ok());
}

#[tokio::test]
async fn reset_without_available_credit_never_calls_consume() {
    let context = QuotaTestContext::new(0, AuthenticationMode::Accepted).await;

    assert!(matches!(
        context.service.reset_quota(context.account_id).await,
        Err(OAuthQuotaError::NoResetCredits)
    ));
    assert_eq!(context.transport.consume_calls(), 0);
}

#[tokio::test]
async fn quota_query_refreshes_once_after_authentication_rejection() {
    let context = QuotaTestContext::new(1, AuthenticationMode::RejectOnce).await;

    context
        .service
        .query_quota(context.account_id)
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
        context.service.query_quota(context.account_id).await,
        Err(OAuthQuotaError::AuthenticationFailed)
    ));
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(context.transport.usage_authorizations().len(), 2);
}

#[tokio::test]
async fn a_failed_token_refresh_does_not_claim_the_account_is_invalid() {
    let context = QuotaTestContext::new(1, AuthenticationMode::RefreshRejected).await;

    assert!(matches!(
        context.service.query_quota(context.account_id).await,
        Err(OAuthQuotaError::AuthenticationRefreshFailed)
    ));
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(context.transport.usage_calls(), 1);
}

#[tokio::test]
async fn invalid_grant_marks_the_account_authentication_as_failed() {
    let context = QuotaTestContext::new(1, AuthenticationMode::RefreshInvalidGrant).await;

    assert!(matches!(
        context.service.query_quota(context.account_id).await,
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
async fn rejected_access_token_without_refresh_token_is_authentication_failed() {
    let context = QuotaTestContext::new_without_refresh_token().await;

    assert!(matches!(
        context.service.query_quota(context.account_id).await,
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
        .query_quota(context.account_id)
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
    drop(snapshot);

    context.transport.set_codex_exhausted(false);
    let available = context
        .service
        .query_quota(context.account_id)
        .await
        .expect("available quota snapshot");
    assert!(available.usage.account_status.is_none());
    assert_eq!(generation.health().availability("gpt-5.5"), Ok(()));
}

#[tokio::test]
async fn concurrent_resets_serialize_and_only_consume_the_last_credit_once() {
    let context = QuotaTestContext::new_blocking_reset(1).await;
    let first_service = Arc::clone(&context.service);
    let id = context.account_id;
    let first = tokio::spawn(async move { first_service.reset_quota(id).await });
    context.transport.wait_for_consume().await;

    let second_service = Arc::clone(&context.service);
    let second = tokio::spawn(async move { second_service.reset_quota(id).await });
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
