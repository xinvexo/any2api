use any2api_domain::{
    RetrySafety, RoutingCredentialId, UpstreamErrorClassification, UpstreamErrorKind,
    UpstreamQuotaExhaustion,
};
use any2api_provider::api::OAuthQuotaTokenBalanceSource;
use http::Method;

use super::{
    test_support::{AuthenticationMode, QuotaTestContext},
    types::OAuthQuotaError,
};
use crate::health::ReliabilityPolicy;

#[tokio::test]
async fn grok_query_reads_billing_and_current_subscription_over_direct_proxy() {
    let context = QuotaTestContext::new_grok().await;

    let quota = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("Grok quota query");

    assert_eq!(
        quota
            .usage
            .rate_limit
            .as_ref()
            .and_then(|limit| limit.windows.first())
            .map(|window| (window.used_percent, window.limit_window_seconds)),
        Some((37.5, Some(604_800)))
    );
    assert_eq!(
        quota.usage.subscription_tier.as_deref(),
        Some("SuperGrokPro")
    );
    let status = quota.usage.account_status.as_ref().expect("account status");
    assert_eq!(
        status.user_blocked_reason.as_deref(),
        Some("BLOCKED_REASON_BILLING")
    );
    assert_eq!(status.team_blocked_reasons, ["BLOCKED_REASON_NO_LOGS"]);
    assert!(status.quota_exhaustion.is_none());
    assert!(quota.usage.reset_credits.is_none());
    let captured = context.transport.captured();
    assert_eq!(
        captured
            .iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        [
            "/v1/billing?format=credits",
            "/v1/user?include=subscription"
        ]
    );
    for request in captured {
        assert_eq!(request.authorization.as_deref(), Some("Bearer grok-access"));
        assert_eq!(request.grok_token_auth.as_deref(), Some("xai-grok-cli"));
        assert_eq!(request.grok_client_version.as_deref(), Some("0.2.112"));
        assert_eq!(request.grok_user_id.as_deref(), Some("grok-subject"));
        assert_eq!(request.grok_client_mode.as_deref(), Some("interactive"));
        assert_eq!(request.proxy_id, any2api_domain::ProxyProfileId::DIRECT);
        assert_eq!(
            request.strict_ssrf,
            context.snapshots.load().settings().upstream().strict_ssrf()
        );
    }
}

#[tokio::test]
async fn grok_free_billing_remains_read_only_and_leaves_token_balance_unknown() {
    let context = QuotaTestContext::new_grok_unified().await;

    let quota = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("Grok unified quota query");

    assert!(quota.usage.rate_limit.is_none());
    let billing = quota.usage.billing.expect("billing");
    assert_eq!(billing.prepaid_balance_minor, Some(2500));
    assert_eq!(billing.on_demand_used_minor, Some(0));
    assert_eq!(billing.on_demand_cap_minor, Some(0));
    assert_eq!(quota.usage.subscription_tier.as_deref(), Some("Free"));
    assert!(quota.usage.token_balance.is_none());
    let status = quota.usage.account_status.expect("account status");
    assert!(status.user_blocked_reason.is_none());
    assert!(status.team_blocked_reasons.is_empty());
    let captured = context.transport.captured();
    assert_eq!(captured.len(), 2);
    assert!(captured.iter().all(|request| request.method == Method::GET));
    assert_eq!(captured[0].path, "/v1/billing?format=credits");
    assert_eq!(captured[1].path, "/v1/user?include=subscription");
}

#[tokio::test]
async fn grok_read_only_quota_preserves_numeric_data_plane_exhaustion() {
    let context = QuotaTestContext::new_grok_unified().await;
    let snapshot = context.snapshots.load();
    let binding = snapshot
        .credential_runtime(RoutingCredentialId::oauth_account(context.account_id))
        .expect("credential runtime");
    binding.generation().health().record(
        "grok-4.5",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::QuotaExhausted,
            RetrySafety::RejectedBeforeExecution,
            None,
        )
        .with_quota_exhaustion(UpstreamQuotaExhaustion::new(1_065_387, 1_000_000)),
        &ReliabilityPolicy::from_settings(snapshot.settings().reliability()),
    );
    drop(snapshot);

    let quota = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("Grok quota query");
    let status = quota.usage.account_status.expect("account status");
    assert_eq!(
        status.quota_exhaustion.expect("exhaustion").used,
        Some(1_065_387)
    );
    let balance = quota.usage.token_balance.expect("observed token balance");
    assert_eq!(balance.source, OAuthQuotaTokenBalanceSource::Upstream);
    assert_eq!(balance.used, 1_065_387);
    assert_eq!(balance.limit, 1_000_000);
    assert_eq!(balance.remaining, 0);
    assert_eq!(balance.window_seconds, None);
}

#[tokio::test]
async fn grok_read_only_quota_preserves_non_numeric_data_plane_exhaustion() {
    let context = QuotaTestContext::new_grok_unified().await;
    let snapshot = context.snapshots.load();
    let binding = snapshot
        .credential_runtime(RoutingCredentialId::oauth_account(context.account_id))
        .expect("credential runtime");
    binding.generation().health().record(
        "grok-4.5",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::QuotaExhausted,
            RetrySafety::RejectedBeforeExecution,
            None,
        ),
        &ReliabilityPolicy::from_settings(snapshot.settings().reliability()),
    );
    drop(snapshot);

    let quota = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("Grok quota query");

    assert!(quota.usage.token_balance.is_none());
    assert!(
        quota
            .usage
            .account_status
            .expect("account status")
            .quota_exhaustion
            .is_some()
    );
}

#[tokio::test]
async fn grok_quota_retries_one_unauthorized_response_then_verifies_authentication() {
    let context =
        QuotaTestContext::new_grok_with_authentication(AuthenticationMode::RejectOnce).await;

    let quota = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("Grok quota after refresh");

    assert!(quota.usage.account_status.is_some());
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(context.transport.usage_calls(), 2);
}

#[tokio::test]
async fn grok_quota_distinguishes_invalid_and_restricted_accounts() {
    let invalid =
        QuotaTestContext::new_grok_with_authentication(AuthenticationMode::AlwaysReject).await;
    assert!(matches!(
        invalid.service.refresh_quota(invalid.account_id).await,
        Err(OAuthQuotaError::AuthenticationFailed)
    ));
    assert_eq!(invalid.transport.refresh_calls(), 1);

    let restricted =
        QuotaTestContext::new_grok_with_authentication(AuthenticationMode::AlwaysForbidden).await;
    assert!(matches!(
        restricted
            .service
            .refresh_quota(restricted.account_id)
            .await,
        Err(OAuthQuotaError::AccountRestricted)
    ));
    assert_eq!(restricted.transport.refresh_calls(), 0);
}
