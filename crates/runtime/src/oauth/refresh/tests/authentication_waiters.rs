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

#[tokio::test]
async fn authentication_refresh_keeps_singleflight_until_publication_after_cancellation() {
    let transport = Arc::new(BlockingRefreshTransport::new());
    let context = RefreshTestContext::with_account(Arc::clone(&transport)).await;
    let id = context.account_id.expect("OAuth account");
    let publish_guard = context.snapshots.acquire_publish().await;

    let refresher = Arc::clone(&context.refresher);
    let first =
        tokio::spawn(async move { refresher.refresh_after_authentication_failure(id, 1).await });
    transport.wait_until_started().await;
    transport.release();
    tokio::time::timeout(std::time::Duration::from_secs(1), async {
        while context._runtime.lifecycle().background_task_count() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("refresh publication started");

    first.abort();
    let Err(error) = first.await else {
        panic!("refresh waiter must be cancelled");
    };
    assert!(error.is_cancelled());
    let mut waiter = Box::pin(
        context
            .refresher
            .refresh_after_authentication_failure(id, 1),
    );
    assert!(futures_util::poll!(waiter.as_mut()).is_pending());
    assert_eq!(transport.calls(), 1);

    drop(publish_guard);
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
        .await
        .expect("refresh waiter completion");
    let OAuthAuthenticationRefreshResult::Refreshed(snapshot) = result else {
        panic!("waiter must observe the published token");
    };
    assert_eq!(
        snapshot
            .oauth_accounts()
            .get(id)
            .expect("account")
            .token_version(),
        2
    );
    assert_eq!(transport.calls(), 1);
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
