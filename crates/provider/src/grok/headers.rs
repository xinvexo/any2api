use http::{HeaderMap, HeaderValue};

use crate::{
    ProviderError,
    api::ProviderRequestHeaderContext,
    header_policy::{insert_default, project},
};

const REQUEST_HEADERS: &[&str] = &[
    "user-agent",
    "x-grok-client-mode",
    "x-grok-client-version",
    "x-grok-client-identifier",
    "x-grok-client-surface",
    "x-grok-conv-id",
    "x-grok-req-id",
    "x-grok-session-id",
    "x-grok-agent-id",
    "x-grok-turn-id",
    "traceparent",
    "tracestate",
];

const RESPONSE_HEADERS: &[&str] = &[
    "content-encoding",
    "content-type",
    "x-request-id",
    "request-id",
    "x-grok-context-window",
    "x-grok-max-completion-tokens",
    "x-grok-doom-loop-check",
    "x-models-etag",
    "x-should-retry",
    "retry-after",
];

pub(crate) fn request(
    context: ProviderRequestHeaderContext<'_>,
) -> Result<HeaderMap, ProviderError> {
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
        headers.extend(project(context.client_headers, REQUEST_HEADERS, &[]));
    }
    if context.oauth {
        let model = HeaderValue::from_str(context.upstream_model)
            .map_err(|_| ProviderError::InvalidResponse("invalid upstream model header".into()))?;
        headers.insert("x-grok-model-override", model);
    }
    Ok(headers)
}

pub(crate) fn response(upstream: &HeaderMap) -> HeaderMap {
    project(upstream, RESPONSE_HEADERS, &["x-ratelimit-"])
}
