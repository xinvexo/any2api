use any2api_contract_tests::TestApplication;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn spa_fallback_only_serves_registered_management_deep_links() {
    let fixture = TestApplication::new().await;
    let app = fixture.router();

    for path in ["/", "/providers", "/logs/request-id", "/settings/providers"] {
        let (status, body) = get(&app, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(body.contains("any2api shell"), "{path}");
    }

    for path in [
        "/definitely-missing",
        "/wp-login.php",
        "/settings/providers/extra",
    ] {
        let (status, body) = get(&app, path).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(!body.contains("any2api shell"), "{path}");
    }
}

async fn get(app: &axum::Router, path: &str) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("web request"),
        )
        .await
        .expect("web response");
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("web response body")
        .to_bytes();
    (
        status,
        String::from_utf8(body.to_vec()).expect("UTF-8 Web body"),
    )
}
