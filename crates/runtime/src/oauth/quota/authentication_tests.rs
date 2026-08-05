use any2api_domain::RoutingCredentialId;

use super::{
    OAuthQuotaError,
    test_support::{AuthenticationMode, QuotaTestContext},
};
use crate::{
    health::HealthAcquireError,
    oauth::refresh::{OAuthRefreshFailureReason, OAuthRefreshFailureStage},
};

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
    assert_eq!(current_token_version(&context), 2);
}

#[tokio::test]
async fn a_second_quota_401_records_the_post_refresh_verification_stage() {
    let context = QuotaTestContext::new(1, AuthenticationMode::AlwaysReject).await;
    let mut diagnostic_changes = context.service.subscribe_refresh_failure_changes();

    assert!(matches!(
        context.service.refresh_quota(context.account_id).await,
        Err(OAuthQuotaError::RefreshedAccessTokenRejected(failure))
            if failure.reason() == OAuthRefreshFailureReason::RefreshedAccessTokenRejected
                && failure.stage() == OAuthRefreshFailureStage::VerifyAuthentication
                && failure.upstream_status() == Some(401)
    ));
    diagnostic_changes
        .changed()
        .await
        .expect("refresh diagnostic change");
    let current_version = current_token_version(&context);
    assert_eq!(current_version, 2);
    assert_eq!(
        context
            .service
            .refresh_failure(context.account_id, current_version)
            .map(|failure| failure.reason()),
        Some(OAuthRefreshFailureReason::RefreshedAccessTokenRejected)
    );
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(context.transport.usage_authorizations().len(), 2);
}

#[tokio::test]
async fn a_failed_token_refresh_does_not_claim_the_account_is_invalid() {
    let context = QuotaTestContext::new(1, AuthenticationMode::RefreshRejected).await;

    assert!(matches!(
        context.service.refresh_quota(context.account_id).await,
        Err(OAuthQuotaError::TokenRefreshFailed(failure))
            if failure.reason() == OAuthRefreshFailureReason::UpstreamRejected
                && failure.stage() == OAuthRefreshFailureStage::TokenEndpoint
                && failure.upstream_status() == Some(503)
                && !failure.reauthorization_required()
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
        Err(OAuthQuotaError::RefreshPermanentlyRejected(failure))
            if failure.reason() == OAuthRefreshFailureReason::InvalidGrant
                && failure.reauthorization_required()
    ));
    assert_permanent_health(&context);
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(context.transport.usage_calls(), 1);
}

#[tokio::test]
async fn reused_refresh_token_is_permanent_and_is_submitted_only_once() {
    let context = QuotaTestContext::new(1, AuthenticationMode::RefreshTokenReused).await;

    for _ in 0..2 {
        assert!(matches!(
            context.service.refresh_quota(context.account_id).await,
            Err(OAuthQuotaError::RefreshPermanentlyRejected(failure))
                if failure.reason() == OAuthRefreshFailureReason::RefreshTokenReused
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
        Err(OAuthQuotaError::RefreshTokenMissing(failure))
            if failure.reason() == OAuthRefreshFailureReason::RefreshTokenMissing
                && failure.stage() == OAuthRefreshFailureStage::Preflight
    ));
    assert_permanent_health(&context);
    assert_eq!(context.transport.refresh_calls(), 0);
}

fn current_token_version(context: &QuotaTestContext) -> u64 {
    context
        .snapshots
        .load()
        .oauth_accounts()
        .get(context.account_id)
        .expect("OAuth account")
        .token_version()
}

fn assert_permanent_health(context: &QuotaTestContext) {
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
}
