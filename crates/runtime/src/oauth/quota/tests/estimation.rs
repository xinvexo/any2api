use any2api_domain::{
    CompletedRequestLog, ConfigRevision, MAX_REQUEST_LOG_ROWS, OAuthAccountId, ProtocolDialect,
    ProtocolOperation, ProxyProfileId, RequestId, RequestLog,
};
use any2api_storage::api::{OAuthQuotaEstimationRepository, RequestLogRepository};
use uuid::Uuid;

use super::super::test_support::{AuthenticationMode, QuotaTestContext};

#[tokio::test]
async fn first_refresh_estimates_from_the_full_retained_request_log_window() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;
    let mut priced_failure = usage_record(context.account_id, unix_now_ms(), Some(2_000), Some(0));
    priced_failure.request.status_code = 502;
    let unpriced = usage_record(context.account_id, unix_now_ms(), None, None);
    context
        .storage
        .append_request_logs(&[priced_failure, unpriced], MAX_REQUEST_LOG_ROWS)
        .await
        .expect("request log");

    let quota = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("quota query");

    assert_eq!(quota.usd_estimates.len(), 1);
    let estimate = &quota.usd_estimates[0];
    assert!((estimate.estimated_used_usd - 0.01).abs() < f64::EPSILON);
    assert!((estimate.estimated_capacity_usd - 0.04).abs() < f64::EPSILON);
    assert_eq!(estimate.sample_used_percent, 25.0);
    assert_eq!(estimate.unpriced_request_count, 1);
}

#[tokio::test]
async fn refresh_after_reset_only_prices_request_logs_at_or_after_the_reset_boundary() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;
    context
        .storage
        .append_request_logs(
            &[usage_record(
                context.account_id,
                unix_now_ms(),
                Some(2_000),
                Some(0),
            )],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("pre-reset request log");
    context
        .service
        .reset_quota(context.account_id, Uuid::new_v4())
        .await
        .expect("quota reset");
    let boundary = context
        .storage
        .load_oauth_quota_reset_boundary(context.account_id)
        .await
        .expect("reset boundary")
        .expect("persisted reset boundary");
    context
        .storage
        .append_request_logs(
            &[usage_record(
                context.account_id,
                boundary,
                Some(1_000),
                Some(0),
            )],
            MAX_REQUEST_LOG_ROWS,
        )
        .await
        .expect("post-reset request log");

    let quota = context
        .service
        .refresh_quota(context.account_id)
        .await
        .expect("post-reset quota query");

    assert_eq!(quota.usd_estimates.len(), 1);
    assert!((quota.usd_estimates[0].sample_cost_usd - 0.005).abs() < f64::EPSILON);
}

fn usage_record(
    id: OAuthAccountId,
    started_at_ms: u64,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
) -> CompletedRequestLog {
    CompletedRequestLog {
        request: RequestLog {
            request_id: RequestId::new(),
            started_at_ms,
            client_ip: "127.0.0.1".parse().expect("client IP"),
            config_revision: ConfigRevision::INITIAL,
            gateway_api_key_id: None,
            ingress_protocol: ProtocolDialect::OpenAiResponses,
            operation: ProtocolOperation::Responses,
            public_model: Some("gpt-5.5".into()),
            thinking_level: None,
            provider_endpoint_id: None,
            credential_id: None,
            oauth_account_id: Some(id),
            proxy_profile_id: Some(ProxyProfileId::DIRECT),
            status_code: 200,
            error_class: None,
            error_message: None,
            attempt_count: 1,
            latency_ms: 1,
            first_token_ms: None,
            input_tokens,
            output_tokens,
            cache_read_tokens: None,
            is_stream: false,
        },
        attempts: Vec::new(),
    }
}

fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Unix time")
        .as_millis() as u64
}
