use std::sync::Arc;

use http::StatusCode;

use super::{BlockingRefreshTransport, RefreshTestContext};
use crate::oauth::refresh::{
    OAuthAuthenticationRefreshResult, OAuthRefreshFailureReason, OAuthRefreshFailureStage,
};

#[tokio::test]
async fn exact_failure_is_shared_with_authentication_waiters() {
    let transport = Arc::new(BlockingRefreshTransport::with_status(
        StatusCode::SERVICE_UNAVAILABLE,
    ));
    let context = RefreshTestContext::with_account(Arc::clone(&transport)).await;
    let id = context.account_id.expect("OAuth account");

    let first_refresher = Arc::clone(&context.refresher);
    let first = tokio::spawn(async move {
        first_refresher
            .refresh_after_authentication_failure(id, 1)
            .await
    });
    transport.wait_until_started().await;
    let waiters = (0..4)
        .map(|_| {
            let refresher = Arc::clone(&context.refresher);
            tokio::spawn(async move { refresher.refresh_after_authentication_failure(id, 1).await })
        })
        .collect::<Vec<_>>();
    transport.release();
    transport.wait_until_started().await;
    transport.release();

    assert_exact_endpoint_rejection(first.await.expect("first refresh"));
    for waiter in waiters {
        assert_exact_endpoint_rejection(waiter.await.expect("refresh waiter"));
    }
    assert_eq!(transport.calls(), 2);
}

fn assert_exact_endpoint_rejection(result: OAuthAuthenticationRefreshResult) {
    assert!(matches!(
        result,
        OAuthAuthenticationRefreshResult::Failed(failure)
            if failure.stage() == OAuthRefreshFailureStage::TokenEndpoint
                && failure.reason() == OAuthRefreshFailureReason::UpstreamRejected
                && failure.upstream_status() == Some(503)
    ));
}
