use std::sync::LazyLock;

use any2api_domain::ProviderKind;
use http::{HeaderMap, HeaderName, HeaderValue};

use crate::{
    ProviderError,
    api::ProviderRequestContext,
    header_policy::{insert_default, ordered_names, project},
};

static REQUEST_HEADERS: LazyLock<Vec<HeaderName>> = LazyLock::new(|| {
    ordered_names(&[
        "x-grok-conv-id",
        "x-grok-req-id",
        "x-grok-session-id",
        "x-grok-agent-id",
        "x-grok-turn-id",
        "user-agent",
        "x-grok-client-mode",
        "x-grok-client-version",
        "x-grok-client-identifier",
        "x-grok-client-surface",
        "traceparent",
        "tracestate",
    ])
});

static RESPONSE_HEADERS: LazyLock<Vec<HeaderName>> = LazyLock::new(|| {
    ordered_names(&[
        "content-encoding",
        "content-type",
        "x-request-id",
        "request-id",
        "retry-after",
        "x-should-retry",
        "x-grok-context-window",
        "x-grok-max-completion-tokens",
        "x-grok-doom-loop-check",
        "x-models-etag",
    ])
});

pub(crate) fn request(context: ProviderRequestContext<'_>) -> Result<HeaderMap, ProviderError> {
    let mut headers = HeaderMap::new();
    insert_default(
        &mut headers,
        "user-agent",
        "grok-shell/0.2.112 (macos; aarch64)",
    );
    insert_default(&mut headers, "x-grok-client-version", "0.2.112");
    insert_default(&mut headers, "x-grok-client-identifier", "grok-shell");
    if context.oauth {
        insert_default(&mut headers, "x-grok-client-mode", "interactive");
        headers.insert("x-xai-token-auth", HeaderValue::from_static("xai-grok-cli"));
        headers.insert(
            "x-authenticateresponse",
            HeaderValue::from_static("authenticate-response"),
        );
    }
    if context.ingress_dialect == context.upstream_operation.dialect() {
        headers.extend(project(context.client_headers, REQUEST_HEADERS.iter(), &[]));
    }
    if context.oauth {
        let model = oauth_model_header(context.upstream_model)?;
        headers.insert("x-grok-model-override", model);
    }
    Ok(headers)
}

fn oauth_model_header(model: &str) -> Result<HeaderValue, ProviderError> {
    HeaderValue::from_bytes(model.as_bytes()).map_err(|_| ProviderError::UnsupportedOAuthModel {
        provider: ProviderKind::Grok,
        model: model.to_owned(),
    })
}

pub(crate) fn response(upstream: &HeaderMap) -> HeaderMap {
    project(upstream, RESPONSE_HEADERS.iter(), &["x-ratelimit-"])
}
