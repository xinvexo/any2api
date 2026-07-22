use axum::{
    body::{Body, Bytes},
    http::{
        Method, StatusCode, Uri,
        header::{ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
};

use crate::web_assets::EmbeddedWebAsset;

const CACHE_NO_CACHE: &str = "no-cache";
const CACHE_IMMUTABLE: &str = "public, max-age=31536000, immutable";

pub(crate) fn response(
    method: &Method,
    uri: &Uri,
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
        .or_else(|| (!requested.starts_with("assets/")).then(|| find(assets, "index.html"))?);
    let Some(asset) = asset else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let bytes = asset.bytes();
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type(asset.path()))
        .header(CACHE_CONTROL, cache_control(asset.path()))
        .header(CONTENT_LENGTH, bytes.len())
        .body(if method == Method::HEAD {
            Body::empty()
        } else {
            Body::from(Bytes::from_static(bytes))
        })
        .expect("embedded web response headers are valid")
}

fn find(assets: &'static [EmbeddedWebAsset], path: &str) -> Option<EmbeddedWebAsset> {
    assets.iter().copied().find(|asset| asset.path() == path)
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
            Method, StatusCode, Uri,
            header::{CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE},
        },
    };
    use http_body_util::BodyExt;

    use super::response;
    use crate::web_assets::EmbeddedWebAsset;

    const ASSETS: &[EmbeddedWebAsset] = &[
        EmbeddedWebAsset::new("assets/app-123.js", b"console.log('ok')"),
        EmbeddedWebAsset::new("assets/app-123.css", b"body{}"),
        EmbeddedWebAsset::new("index.html", b"<main>embedded</main>"),
    ];

    #[tokio::test]
    async fn serves_index_deep_links_and_exact_assets() {
        let index = response(&Method::GET, &Uri::from_static("/"), ASSETS);
        assert_eq!(index.status(), StatusCode::OK);
        assert_eq!(index.headers()[CONTENT_TYPE], "text/html; charset=utf-8");
        assert_eq!(index.headers()[CACHE_CONTROL], "no-cache");
        assert_eq!(body(index).await.as_ref(), b"<main>embedded</main>");

        let deep_link = response(&Method::GET, &Uri::from_static("/settings"), ASSETS);
        assert_eq!(body(deep_link).await.as_ref(), b"<main>embedded</main>");

        let script = response(
            &Method::GET,
            &Uri::from_static("/assets/app-123.js"),
            ASSETS,
        );
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
    async fn head_missing_assets_and_writes_have_explicit_semantics() {
        let head = response(
            &Method::HEAD,
            &Uri::from_static("/assets/app-123.css"),
            ASSETS,
        );
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers()[CONTENT_LENGTH], "6");
        assert!(body(head).await.is_empty());

        let missing = response(
            &Method::GET,
            &Uri::from_static("/assets/missing.js"),
            ASSETS,
        );
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        let write = response(&Method::POST, &Uri::from_static("/settings"), ASSETS);
        assert_eq!(write.status(), StatusCode::METHOD_NOT_ALLOWED);
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
