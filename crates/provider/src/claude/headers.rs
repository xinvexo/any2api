use http::HeaderMap;

use crate::{
    ProviderError,
    api::ProviderRequestHeaderContext,
    header_policy::{insert_default, project},
};

const REQUEST_HEADERS: &[&str] = &[
    "anthropic-version",
    "anthropic-beta",
    "anthropic-mcp-client-capabilities",
    "user-agent",
    "x-app",
    "x-client-request-id",
    "x-claude-code-session-id",
    "anthropic-usage-limit",
    "anthropic-dangerous-direct-browser-access",
    "anthropic-client-platform",
    "x-anthropic-additional-protection",
    "x-claude-remote-container-id",
    "x-claude-remote-session-id",
    "x-claude-code-agent-id",
    "x-claude-code-parent-agent-id",
    "traceparent",
    "tracestate",
];

const RESPONSE_HEADERS: &[&str] = &[
    "content-encoding",
    "content-type",
    "request-id",
    "x-request-id",
    "retry-after",
    "retry-after-ms",
    "x-should-retry",
    "anthropic-usage-limit",
    "cf-ray",
];

pub(crate) fn request(
    context: ProviderRequestHeaderContext<'_>,
) -> Result<HeaderMap, ProviderError> {
    let mut headers = HeaderMap::new();
    insert_default(&mut headers, "user-agent", "claude-code/2.1.220");
    insert_default(&mut headers, "x-app", "cli");
    insert_default(&mut headers, "anthropic-version", "2023-06-01");
    if context.ingress_dialect == context.upstream_operation.dialect() {
        headers.extend(project(
            context.client_headers,
            REQUEST_HEADERS,
            &["x-stainless-"],
        ));
    }
    Ok(headers)
}

pub(crate) fn response(upstream: &HeaderMap) -> HeaderMap {
    project(upstream, RESPONSE_HEADERS, &["anthropic-ratelimit-"])
}
