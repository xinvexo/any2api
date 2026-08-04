use std::sync::Arc;

use bytes::Bytes;

use super::{BlockingRefreshTransport, RefreshTestContext};

#[tokio::test]
async fn refresh_without_expiry_is_rejected_without_publishing() {
    let transport = Arc::new(BlockingRefreshTransport::with_response(Bytes::from_static(
        br#"{"access_token":"new-access"}"#,
    )));
    let context = RefreshTestContext::with_account(Arc::clone(&transport)).await;
    let id = context.account_id.expect("OAuth account");
    transport.release();

    context
        .refresher
        .refresh_if_due(id, 1)
        .await
        .expect_err("missing expires_in must reject the token response");
    let published = context.snapshots.load();
    let account = published
        .oauth_accounts()
        .get(id)
        .expect("unchanged account");
    let token = published
        .oauth_token_material(id)
        .expect("unchanged OAuth token");
    assert_eq!(
        (account.token_version(), account.expires_at()),
        (1, Some(0))
    );
    assert_eq!(
        (token.access_token(), token.expires_at()),
        ("old-access", Some(0))
    );
}
