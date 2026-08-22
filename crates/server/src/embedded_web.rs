use axum::{
    body::{Body, Bytes},
    http::{
        HeaderMap, Method, StatusCode, Uri,
        header::{ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
    },
    response::{IntoResponse, Response},
};

use crate::{
    web_assets::{EmbeddedWebAsset, is_management_deep_link},
    web_security_headers,
};

const CACHE_NO_CACHE: &str = "no-cache";
const CACHE_IMMUTABLE: &str = "public, max-age=31536000, immutable";

pub(crate) fn response(
    method: &Method,
    uri: &Uri,
    request_headers: &HeaderMap,
    assets: &'static [EmbeddedWebAsset],
) -> Response {
    let mut response = response_without_security_headers(method, uri, request_headers, assets);
    web_security_headers::insert(response.headers_mut());
    response
}

fn response_without_security_headers(
    method: &Method,
    uri: &Uri,
    request_headers: &HeaderMap,
    assets: &'static [EmbeddedWebAsset],
) -> Response {
    if method != Method::GET && method != Method::HEAD {
        let mut response = StatusCode::METHOD_NOT_ALLOWED.into_response();
        response
            .headers_mut()
            .insert(ALLOW, "GET, HEAD".parse().expect("static allow header"));
        return response;
    }

    let requested = uri.path().trim_start_matches('/');
    let requested = if requested.is_empty() {
        "index.html"
    } else {
        requested
    };
    let asset = find(assets, requested)
        .or_else(|| is_management_deep_link(uri.path()).then(|| find(assets, "index.html"))?);
    let Some(asset) = asset else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if matches_etag(request_headers, asset.etag()) {
        return Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(ETAG, asset.etag())
            .header(CACHE_CONTROL, cache_control(asset.path()))
            .body(Body::empty())
            .expect("embedded web response headers are valid");
    }

    let bytes = asset.bytes();
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type(asset.path()))
        .header(CACHE_CONTROL, cache_control(asset.path()))
        .header(ETAG, asset.etag())
        .header(CONTENT_LENGTH, bytes.len())
        .body(if method == Method::HEAD {
            Body::empty()
        } else {
            Body::from(Bytes::from_static(bytes))
        })
        .expect("embedded web response headers are valid")
}

fn find(assets: &'static [EmbeddedWebAsset], path: &str) -> Option<EmbeddedWebAsset> {
    assets
        .binary_search_by(|asset| asset.path().cmp(path))
        .ok()
        .map(|index| assets[index])
}

fn matches_etag(headers: &HeaderMap, expected: &str) -> bool {
    headers
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|candidate| {
            candidate == "*"
                || candidate == expected
                || candidate.strip_prefix("W/") == Some(expected)
        })
}

fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        CACHE_IMMUTABLE
    } else {
        CACHE_NO_CACHE
    }
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, Bytes},
        http::{
            HeaderMap, HeaderValue, Method, StatusCode, Uri,
            header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_NONE_MATCH},
        },
    };
    use http_body_util::BodyExt;

    use super::{CACHE_IMMUTABLE, CACHE_NO_CACHE, find, response};
    use crate::web_assets::EmbeddedWebAsset;

    const ASSETS: &[EmbeddedWebAsset] = &[
        EmbeddedWebAsset::new("assets/app-123.css", b"body{}", "\"css-etag\""),
        EmbeddedWebAsset::new("assets/app-123.js", b"console.log('ok')", "\"script-etag\""),
        EmbeddedWebAsset::new("index.html", b"<main>embedded</main>", "\"index-etag\""),
    ];

    #[tokio::test]
    async fn serves_index_deep_links_and_exact_assets() {
        let index = serve(&Method::GET, &Uri::from_static("/"));
        assert_eq!(index.status(), StatusCode::OK);
        assert_eq!(index.headers()[CONTENT_TYPE], "text/html; charset=utf-8");
        assert_eq!(index.headers()[CACHE_CONTROL], "no-cache");
        assert_eq!(index.headers()["referrer-policy"], "no-referrer");
        let policy = index.headers()["content-security-policy"]
            .to_str()
            .expect("content security policy");
        assert!(policy.contains("frame-ancestors 'none'"));
        assert!(!policy.contains("upgrade-insecure-requests"));
        assert!(!index.headers().contains_key("strict-transport-security"));
        assert_eq!(index.headers()[ETAG], "\"index-etag\"");
        assert_eq!(body(index).await.as_ref(), b"<main>embedded</main>");

        let deep_link = serve(&Method::GET, &Uri::from_static("/settings"));
        assert_eq!(deep_link.headers()[ETAG], "\"index-etag\"");
        assert_eq!(body(deep_link).await.as_ref(), b"<main>embedded</main>");

        let script = serve(&Method::GET, &Uri::from_static("/assets/app-123.js"));
        assert_eq!(
            script.headers()[CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            script.headers()[CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        assert_eq!(body(script).await.as_ref(), b"console.log('ok')");
    }

    #[tokio::test]
    async fn unknown_paths_missing_assets_and_writes_have_explicit_semantics() {
        let head = serve(&Method::HEAD, &Uri::from_static("/assets/app-123.css"));
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers()[CONTENT_LENGTH], "6");
        assert!(body(head).await.is_empty());

        let missing = serve(&Method::GET, &Uri::from_static("/assets/missing.js"));
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        for path in ["/definitely-missing", "/wp-login.php"] {
            let unknown = serve(&Method::GET, &path.parse().expect("unknown URI"));
            assert_eq!(unknown.status(), StatusCode::NOT_FOUND, "{path}");
        }

        let write = serve(&Method::POST, &Uri::from_static("/settings"));
        assert_eq!(write.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn conditional_requests_preserve_cache_policy_and_spa_validator() {
        for value in [
            "\"script-etag\"",
            "W/\"script-etag\"",
            "\"other\", W/\"script-etag\"",
            "*",
        ] {
            let response = serve_with_if_none_match(
                &Method::GET,
                &Uri::from_static("/assets/app-123.js"),
                value,
            );
            assert_eq!(response.status(), StatusCode::NOT_MODIFIED, "{value}");
            assert_eq!(response.headers()[ETAG], "\"script-etag\"");
            assert_eq!(response.headers()[CACHE_CONTROL], CACHE_IMMUTABLE);
            assert!(!response.headers().contains_key(CONTENT_LENGTH));
            assert!(body(response).await.is_empty());
        }

        let deep_link = serve_with_if_none_match(
            &Method::HEAD,
            &Uri::from_static("/settings/providers"),
            "\"index-etag\"",
        );
        assert_eq!(deep_link.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(deep_link.headers()[ETAG], "\"index-etag\"");
        assert_eq!(deep_link.headers()[CACHE_CONTROL], CACHE_NO_CACHE);
        assert!(body(deep_link).await.is_empty());

        let changed =
            serve_with_if_none_match(&Method::GET, &Uri::from_static("/"), "\"stale-index\"");
        assert_eq!(changed.status(), StatusCode::OK);
        assert_eq!(body(changed).await.as_ref(), b"<main>embedded</main>");
    }

    #[test]
    fn sorted_assets_are_queried_by_path() {
        assert_eq!(
            find(ASSETS, "assets/app-123.css").map(EmbeddedWebAsset::path),
            Some("assets/app-123.css")
        );
        assert_eq!(
            find(ASSETS, "assets/app-123.js").map(EmbeddedWebAsset::path),
            Some("assets/app-123.js")
        );
        assert_eq!(
            find(ASSETS, "index.html").map(EmbeddedWebAsset::path),
            Some("index.html")
        );
        assert!(find(ASSETS, "missing.txt").is_none());
    }

    fn serve(method: &Method, uri: &Uri) -> axum::response::Response<Body> {
        response(method, uri, &HeaderMap::new(), ASSETS)
    }

    fn serve_with_if_none_match(
        method: &Method,
        uri: &Uri,
        value: &'static str,
    ) -> axum::response::Response<Body> {
        let mut headers = HeaderMap::new();
        headers.insert(IF_NONE_MATCH, HeaderValue::from_static(value));
        response(method, uri, &headers, ASSETS)
    }

    async fn body(response: axum::response::Response<Body>) -> Bytes {
        response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes()
    }
}
