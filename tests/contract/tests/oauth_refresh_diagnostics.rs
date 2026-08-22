use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use any2api_contract_tests::TestApplication;
use any2api_domain::{
    OAuthAccountDraft, OAuthAccountId, OAuthProxySelection, ProviderKind, ProxyProfileId,
};
use any2api_runtime::api::{OAuthService, RequestTelemetry};
use any2api_storage::api::OAuthAccountDocument;
use any2api_transport::api::{
    BoxByteStream, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};
use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode},
};
use bytes::Bytes;
use futures_util::stream;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

#[tokio::test]
async fn refresh_rejection_exposes_one_safe_diagnostic_across_error_account_and_event() {
    let fixture = TestApplication::new().await;
    let account_id = OAuthAccountId::new();
    fixture
        .publisher()
        .activate_oauth_account(
            account_id,
            ProviderKind::Codex,
            OAuthAccountDraft::new("Codex OAuth", None, true).expect("OAuth draft"),
            OAuthProxySelection::Global,
            Some("person@example.com".into()),
            None,
            vec!["gpt-5.5".into()],
            OAuthAccountDocument::new(
                ProviderKind::Codex,
                br#"{"access_token":"access-secret","refresh_token":"refresh-secret","id_token":"header.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9wbGFuX3R5cGUiOiJmcmVlIn19.signature","account_id":"account-123","email":"person@example.com"}"#
                    .to_vec()
                    .into(),
            )
            .expect("OAuth document"),
        )
        .await
        .expect("OAuth account");
    let transport = Arc::new(RejectedRefreshTransport::default());
    let oauth = Arc::new(OAuthService::new(
        fixture.components().provider_registry_handle(),
        Arc::clone(&transport) as Arc<dyn TransportManager>,
        fixture.publisher(),
        fixture.storage(),
        Arc::new(RequestTelemetry::disabled()),
    ));
    let state = fixture.state().with_oauth(oauth);
    let (_directory, app, _) = fixture.into_router_with_state(state);
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let response = app
        .clone()
        .oneshot(request(Method::GET, "/api/admin/events", loopback))
        .await
        .expect("event response");
    let mut events = response.into_body();
    for _ in 0..6 {
        next_sse_event(&mut events).await;
    }

    let quota = json_request(
        app.clone(),
        Method::POST,
        &format!("/api/admin/oauth/accounts/{account_id}/quota/refresh"),
        loopback,
    )
    .await;
    assert_eq!(quota.0, StatusCode::BAD_GATEWAY);
    assert_eq!(
        quota.1["error"]["code"],
        "oauth_refresh_permanently_rejected"
    );
    assert_diagnostic(&quota.1["error"]["diagnostic"]);
    let error_body = quota.1.to_string();
    for secret in ["access-secret", "refresh-secret", "account-123"] {
        assert!(!error_body.contains(secret));
    }

    let event = next_sse_event(&mut events).await;
    assert!(event.contains("event: oauth_refresh_diagnostic_changed"));

    let accounts = json_request(app, Method::GET, "/api/admin/oauth/accounts", loopback).await;
    assert_eq!(accounts.0, StatusCode::OK);
    assert_diagnostic(&accounts.1["items"][0]["token_refresh_failure"]);
    let encoded = accounts.1.to_string();
    for secret in ["access-secret", "refresh-secret", "account-123"] {
        assert!(!encoded.contains(secret));
    }
    assert_eq!(transport.calls.load(Ordering::Acquire), 2);
}

fn assert_diagnostic(diagnostic: &Value) {
    assert_eq!(diagnostic["token_version"], 1);
    assert_eq!(diagnostic["trigger"], "authentication_failure");
    assert_eq!(diagnostic["stage"], "token_endpoint");
    assert_eq!(diagnostic["reason"], "refresh_token_reused");
    assert_eq!(diagnostic["upstream_status"], 400);
    assert_eq!(diagnostic["failure_scope"], Value::Null);
    assert_eq!(diagnostic["reauthorization_required"], true);
    assert!(
        diagnostic["occurred_at"]
            .as_i64()
            .is_some_and(|value| value > 0)
    );
}

#[derive(Default)]
struct RejectedRefreshTransport {
    calls: AtomicUsize,
}

#[async_trait]
impl TransportManager for RejectedRefreshTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        assert_eq!(proxy.profile().id(), ProxyProfileId::DIRECT);
        self.calls.fetch_add(1, Ordering::AcqRel);
        let (status, body) = match request.uri.path() {
            "/backend-api/wham/usage" => (StatusCode::UNAUTHORIZED, Bytes::from_static(b"{}")),
            "/oauth/token" => (
                StatusCode::BAD_REQUEST,
                Bytes::from_static(br#"{"error":{"code":"refresh_token_reused"}}"#),
            ),
            path => panic!("unexpected OAuth request path: {path}"),
        };
        let body: BoxByteStream = Box::pin(stream::iter([Ok(body)]));
        Ok(TransportResponse {
            status,
            headers: Default::default(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

fn request(method: Method, uri: &str, remote: SocketAddr) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(remote))
        .body(Body::empty())
        .expect("request")
}

async fn json_request(
    app: Router,
    method: Method,
    uri: &str,
    remote: SocketAddr,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(request(method, uri, remote))
        .await
        .expect("response");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&body).expect("response JSON"),
    )
}

async fn next_sse_event(body: &mut Body) -> String {
    let mut frame = Vec::new();
    while !frame.ends_with(b"\n\n") {
        let next = body.frame().await.expect("SSE frame").expect("SSE body");
        if let Ok(data) = next.into_data() {
            frame.extend_from_slice(&data);
        }
    }
    String::from_utf8(frame).expect("SSE UTF-8")
}
