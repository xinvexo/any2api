use any2api_provider::OAuthQuotaWindowKind;

use super::quota_test_support::QuotaTestContext;

#[tokio::test]
async fn grok_query_uses_one_direct_billing_request_without_reset_credits() {
    let context = QuotaTestContext::new_grok().await;

    let quota = context
        .service
        .query_quota(context.account_id)
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
    assert!(quota.usage.reset_credits.is_none());
    let captured = context.transport.captured();
    assert_eq!(captured.len(), 1);
    let request = &captured[0];
    assert_eq!(request.path, "/v1/billing?format=credits");
    assert_eq!(request.authorization.as_deref(), Some("Bearer grok-access"));
    assert_eq!(request.grok_token_auth.as_deref(), Some("xai-grok-cli"));
    assert_eq!(request.grok_client_version.as_deref(), Some("0.2.93"));
    assert!(request.account_id.is_none());
    assert_eq!(request.proxy_id, any2api_domain::ProxyProfileId::DIRECT);
    assert_eq!(
        request.strict_ssrf,
        context.snapshots.load().settings().upstream().strict_ssrf()
    );
}

#[tokio::test]
async fn grok_unified_billing_falls_back_to_a_header_only_responses_probe() {
    let context = QuotaTestContext::new_grok_unified().await;

    let quota = context
        .service
        .query_quota(context.account_id)
        .await
        .expect("Grok unified quota query");

    let rate_limit = quota.usage.rate_limit.expect("rate limit");
    let requests = &rate_limit.windows[0];
    assert_eq!(requests.kind, OAuthQuotaWindowKind::Requests);
    assert_eq!(requests.used_percent, 25.0);
    assert_eq!(requests.limit_window_seconds, None);
    let tokens = &rate_limit.windows[1];
    assert_eq!(tokens.kind, OAuthQuotaWindowKind::Tokens);
    assert_eq!(tokens.used_percent, 60.0);

    let captured = context.transport.captured();
    assert_eq!(
        captured
            .iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        ["/v1/billing?format=credits", "/v1/responses"]
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&captured[1].body).expect("probe body"),
        serde_json::json!({"model":"grok-4.5","input":"hi","stream":true})
    );
}
