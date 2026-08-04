use std::sync::LazyLock;

use any2api_domain::{ProtocolDialect, RequestBodyEncoding};
use http::{HeaderMap, HeaderName};

use crate::{
    ProviderError,
    api::ProviderRequestHeaderContext,
    header_policy::{insert_default, ordered_names, project},
};

static REQUEST_HEADERS: LazyLock<Vec<HeaderName>> = LazyLock::new(|| {
    ordered_names(&[
        "openai-beta",
        "x-codex-turn-state",
        "x-oai-attestation",
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
        "x-openai-subagent",
        "x-openai-memgen-request",
        "x-responsesapi-include-timing-metrics",
        "x-openai-internal-codex-responses-lite",
        "x-openai-internal-codex-residency",
        "traceparent",
        "tracestate",
    ])
});

static RESPONSE_HEADERS: LazyLock<Vec<HeaderName>> = LazyLock::new(|| {
    ordered_names(&[
        "content-encoding",
        "content-type",
        "x-request-id",
        "x-oai-request-id",
        "request-id",
        "retry-after",
        "x-should-retry",
        "openai-model",
        "x-reasoning-included",
        "x-models-etag",
        "cf-ray",
    ])
});

pub(crate) fn request(
    context: ProviderRequestHeaderContext<'_>,
) -> Result<HeaderMap, ProviderError> {
    let mut headers = HeaderMap::new();
    insert_default(&mut headers, "originator", "codex_cli_rs");
    insert_default(&mut headers, "user-agent", "codex_cli_rs/0.145.0");
    if context.ingress_dialect == context.upstream_operation.dialect() {
        let allow_credential_bound = context.allow_credential_bound;
        let allow_turn_state = context.allow_turn_state;
        let allowed = REQUEST_HEADERS.iter().filter(move |name| {
            (allow_credential_bound || name.as_str() != "x-oai-attestation")
                && (allow_turn_state || name.as_str() != "x-codex-turn-state")
        });
        headers.extend(project(context.client_headers, allowed, &[]));
    }
    Ok(headers)
}

pub(crate) fn response(upstream: &HeaderMap) -> HeaderMap {
    let mut headers = project(
        upstream,
        RESPONSE_HEADERS.iter(),
        &["x-codex-", "x-ratelimit-"],
    );
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
        for _ in 0..64 {
            client.append("x-oai-attestation", HeaderValue::from_static("opaque"));
            client.append("x-codex-turn-state", HeaderValue::from_static("sticky"));
        }
        client.insert("openai-beta", HeaderValue::from_static("responses=v1"));
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
        assert_eq!(projected["openai-beta"], "responses=v1");
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
