use any2api_domain::{ProtocolDialect, RequestBodyEncoding};
use http::HeaderMap;

use crate::{
    ProviderError,
    api::ProviderRequestHeaderContext,
    header_policy::{insert_default, project},
};

const REQUEST_HEADERS: &[&str] = &[
    "user-agent",
    "originator",
    "x-client-request-id",
    "session-id",
    "thread-id",
    "x-codex-installation-id",
    "x-codex-window-id",
    "x-codex-turn-metadata",
    "x-codex-parent-thread-id",
    "x-codex-beta-features",
    "x-codex-turn-state",
    "x-openai-subagent",
    "x-openai-memgen-request",
    "x-responsesapi-include-timing-metrics",
    "x-openai-internal-codex-responses-lite",
    "x-openai-internal-codex-residency",
    "x-oai-attestation",
    "openai-beta",
    "traceparent",
    "tracestate",
];

const RESPONSE_HEADERS: &[&str] = &[
    "x-request-id",
    "x-oai-request-id",
    "request-id",
    "x-models-etag",
    "openai-model",
    "x-reasoning-included",
    "retry-after",
    "x-should-retry",
    "cf-ray",
];

pub(crate) fn request(
    context: ProviderRequestHeaderContext<'_>,
) -> Result<HeaderMap, ProviderError> {
    let mut headers = HeaderMap::new();
    insert_default(&mut headers, "originator", "codex_cli_rs");
    insert_default(&mut headers, "user-agent", "codex_cli_rs/0.145.0");
    if context.ingress_dialect == context.upstream_operation.dialect() {
        headers.extend(project(context.client_headers, REQUEST_HEADERS, &[]));
    }
    if !context.allow_credential_bound {
        headers.remove("x-oai-attestation");
    }
    if !context.allow_turn_state {
        headers.remove("x-codex-turn-state");
    }
    Ok(headers)
}

pub(crate) fn response(upstream: &HeaderMap) -> HeaderMap {
    let mut headers = project(upstream, RESPONSE_HEADERS, &["x-codex-", "x-ratelimit-"]);
    if !headers.contains_key("x-request-id")
        && let Some(value) = headers.get("x-oai-request-id").cloned()
    {
        headers.insert("x-request-id", value);
    }
    headers
}

pub(crate) fn supports_encoding(
    context: ProviderRequestHeaderContext<'_>,
    encoding: RequestBodyEncoding,
) -> bool {
    encoding == RequestBodyEncoding::Zstd
        && context.ingress_dialect == context.upstream_operation.dialect()
        && matches!(
            context.upstream_operation.dialect(),
            ProtocolDialect::OpenAiResponses | ProtocolDialect::OpenAiChatCompletions
        )
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolDialect, ProtocolOperation};
    use http::{HeaderMap, HeaderValue};

    use super::{request, response};
    use crate::api::ProviderRequestHeaderContext;

    #[test]
    fn credential_bound_codex_headers_are_not_replayed_after_a_switch() {
        let mut client = HeaderMap::new();
        client.insert("x-oai-attestation", HeaderValue::from_static("opaque"));
        client.insert("x-codex-turn-state", HeaderValue::from_static("sticky"));
        let context = ProviderRequestHeaderContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "gpt",
            client_headers: &client,
            oauth: true,
            allow_credential_bound: false,
            allow_turn_state: false,
        };
        let projected = request(context).expect("headers");
        assert!(!projected.contains_key("x-oai-attestation"));
        assert!(!projected.contains_key("x-codex-turn-state"));
    }

    #[test]
    fn oai_request_id_is_mirrored_for_clients_that_prefer_x_request_id() {
        let mut upstream = HeaderMap::new();
        upstream.insert(
            "x-oai-request-id",
            HeaderValue::from_static("upstream-oai-request"),
        );

        let projected = response(&upstream);

        assert_eq!(projected["x-oai-request-id"], "upstream-oai-request");
        assert_eq!(projected["x-request-id"], "upstream-oai-request");
    }
}
