use any2api_provider::api::OAuthQuotaWindowKind;

use super::test_support::{AuthenticationMode, QuotaTestContext};

#[tokio::test]
async fn claude_query_uses_one_direct_usage_request_and_keeps_all_windows() {
    let context = QuotaTestContext::new_claude(AuthenticationMode::Accepted).await;

    let quota = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("Claude quota query");

    let rate_limit = quota.usage.rate_limit.expect("rate limit");
    assert_eq!(rate_limit.allowed, None);
    assert_eq!(rate_limit.limit_reached, None);
    assert_eq!(
        rate_limit
            .windows
            .iter()
            .map(|window| (window.id.as_str(), window.kind, window.used_percent))
            .collect::<Vec<_>>(),
        [
            ("five_hour", OAuthQuotaWindowKind::Time, 12.5),
            ("seven_day", OAuthQuotaWindowKind::Time, 34.0),
            ("seven_day_sonnet", OAuthQuotaWindowKind::Time, 56.0),
            (
                "seven_day_overage_included",
                OAuthQuotaWindowKind::Time,
                78.0,
            ),
        ]
    );
    assert!(quota.usage.reset_credits.is_none());

    let captured = context.transport.captured();
    assert_eq!(captured.len(), 1);
    let request = &captured[0];
    assert_eq!(request.path, "/api/oauth/usage");
    assert_eq!(request.authorization.as_deref(), Some("Bearer old-access"));
    assert_eq!(request.anthropic_beta.as_deref(), Some("oauth-2025-04-20"));
    assert_eq!(request.user_agent.as_deref(), Some("claude-code/2.1.7"));
    assert_eq!(request.proxy_id, any2api_domain::ProxyProfileId::DIRECT);
    assert_eq!(
        request.strict_ssrf,
        context.snapshots.load().settings().upstream().strict_ssrf()
    );
}

#[tokio::test]
async fn claude_quota_refreshes_token_once_after_a_401() {
    let context = QuotaTestContext::new_claude(AuthenticationMode::RejectOnce).await;

    context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("Claude quota after refresh");

    assert_eq!(
        context
            .transport
            .captured()
            .iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        ["/api/oauth/usage", "/v1/oauth/token", "/api/oauth/usage"]
    );
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
