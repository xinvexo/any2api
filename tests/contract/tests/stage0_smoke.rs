use std::{fs, net::SocketAddr};

use any2api_contract_tests::TestApplication;
use any2api_server::api::{EmbeddedWebAsset, WebAssets, build_router};
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{
        Request,
        header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH},
    },
};
use http_body_util::BodyExt;
use tower::ServiceExt;

const EMBEDDED_WEB_ASSETS: &[EmbeddedWebAsset] = &[
    EmbeddedWebAsset::new(
        "assets/app.js",
        b"console.log('embedded')",
        "\"embedded-script\"",
    ),
    EmbeddedWebAsset::new(
        "index.html",
        b"<main>embedded shell</main>",
        "\"embedded-index\"",
    ),
];

#[tokio::test]
async fn sqlite_bootstrap_and_health_route_share_the_loaded_revision() {
    let fixture = TestApplication::new().await;
    let web_root = fixture.directory().join("custom-web");
    fs::create_dir(&web_root).expect("web directory");
    fs::create_dir(web_root.join("assets")).expect("asset directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    fs::write(web_root.join("assets/app.js"), "console.log('asset')").expect("web asset");
    let state = fixture.state();
    let app = build_router(state.clone(), web_root);
    let embedded_app = build_router(state, WebAssets::embedded(EMBEDDED_WEB_ASSETS));
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("health response");

    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()[CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(!response.headers().contains_key("content-security-policy"));
    let body = response
        .into_body()
        .collect()
        .await
        .expect("health body")
        .to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).expect("health json");

    assert_eq!(value["status"], "ok");
    assert_eq!(value["application_version"], "0.0.0-dev");
    assert_eq!(value["config_revision"], 1);
    assert_eq!(value["scheduler_epoch"], 0);
    assert_eq!(value["shutdown_phase"], "running");
    assert_eq!(value["active_requests"], 0);
    assert_eq!(value["background_tasks"], 0);

    let deep_link = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/settings")
                .body(Body::empty())
                .expect("deep link request"),
        )
        .await
        .expect("deep link response");
    assert_eq!(deep_link.status(), 200);
    assert_eq!(deep_link.headers()["x-content-type-options"], "nosniff");
    assert_eq!(deep_link.headers()["referrer-policy"], "no-referrer");
    assert!(
        deep_link.headers()["content-security-policy"]
            .to_str()
            .expect("content security policy")
            .contains("frame-ancestors 'none'")
    );
    assert!(
        !deep_link
            .headers()
            .contains_key("strict-transport-security")
    );
    let deep_link_body = deep_link
        .into_body()
        .collect()
        .await
        .expect("deep link body")
        .to_bytes();
    assert!(
        deep_link_body
            .windows(13)
            .any(|part| part == b"any2api shell")
    );

    let log_deep_link = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/logs/11111111-1111-4111-8111-111111111111")
                .body(Body::empty())
                .expect("request log deep link request"),
        )
        .await
        .expect("request log deep link response");
    assert_eq!(log_deep_link.status(), 200);

    let asset = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .expect("asset request"),
        )
        .await
        .expect("asset response");
    assert_eq!(asset.status(), 200);
    assert_eq!(asset.headers()["x-content-type-options"], "nosniff");

    let missing_asset = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/assets/missing.js")
                .body(Body::empty())
                .expect("missing asset request"),
        )
        .await
        .expect("missing asset response");
    assert_eq!(missing_asset.status(), 404);
    assert_eq!(missing_asset.headers()["x-content-type-options"], "nosniff");

    let rejected_write = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/settings")
                .body(Body::empty())
                .expect("static write request"),
        )
        .await
        .expect("static write response");
    assert_eq!(rejected_write.status(), 405);
    assert_eq!(
        rejected_write.headers()["x-content-type-options"],
        "nosniff"
    );

    let embedded_deep_link = embedded_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/settings/providers")
                .body(Body::empty())
                .expect("embedded deep link request"),
        )
        .await
        .expect("embedded deep link response");
    assert_eq!(embedded_deep_link.status(), 200);
    assert_eq!(
        embedded_deep_link.headers()["x-content-type-options"],
        "nosniff"
    );
    assert_eq!(embedded_deep_link.headers()[ETAG], "\"embedded-index\"");
    assert_eq!(embedded_deep_link.headers()[CACHE_CONTROL], "no-cache");

    let embedded_unchanged = embedded_app
        .oneshot(
            Request::builder()
                .uri("/settings/providers")
                .header(IF_NONE_MATCH, "W/\"embedded-index\"")
                .body(Body::empty())
                .expect("conditional embedded deep link request"),
        )
        .await
        .expect("conditional embedded deep link response");
    assert_eq!(embedded_unchanged.status(), 304);
    assert_eq!(
        embedded_unchanged.headers()["x-content-type-options"],
        "nosniff"
    );
    assert_eq!(embedded_unchanged.headers()[ETAG], "\"embedded-index\"");
    assert_eq!(embedded_unchanged.headers()[CACHE_CONTROL], "no-cache");
    assert!(
        embedded_unchanged
            .into_body()
            .collect()
            .await
            .expect("conditional embedded body")
            .to_bytes()
            .is_empty()
    );

    for uri in ["/api", "/api/", "/api/missing"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("missing api request"),
            )
            .await
            .expect("missing api response");
        assert_eq!(response.status(), 404, "unexpected status for {uri}");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    }

    for uri in ["/v1", "/v1/"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41_000))))
                    .body(Body::empty())
                    .expect("public api root request"),
            )
            .await
            .expect("public api root response");
        assert_eq!(response.status(), 401, "unexpected status for {uri}");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    }
}
