use std::sync::LazyLock;

use any2api_domain::{ProtocolDialect, RequestBodyEncoding};
use http::{HeaderMap, HeaderName};

use crate::{
    ProviderError,
    api::ProviderRequestContext,
    header_policy::{ordered_names, project},
    request_header_policy::{
        RequestHeaderOwnership::{BoundTurnState, CredentialOwned, Replayable},
        RequestHeaderRule, project_request, request_header_rules,
    },
};

static REQUEST_HEADERS: LazyLock<Vec<RequestHeaderRule>> = LazyLock::new(|| {
    request_header_rules(&[
        ("openai-beta", Replayable),
        ("x-codex-turn-state", BoundTurnState),
        ("x-oai-attestation", CredentialOwned),
        ("user-agent", Replayable),
        ("originator", Replayable),
        ("x-client-request-id", CredentialOwned),
        ("session-id", CredentialOwned),
        ("thread-id", CredentialOwned),
        ("x-codex-installation-id", CredentialOwned),
        ("x-codex-window-id", CredentialOwned),
        ("x-codex-turn-metadata", CredentialOwned),
        ("x-codex-parent-thread-id", CredentialOwned),
        ("x-codex-beta-features", Replayable),
        ("x-openai-subagent", CredentialOwned),
        ("x-openai-memgen-request", Replayable),
        ("x-responsesapi-include-timing-metrics", Replayable),
        ("x-openai-internal-codex-responses-lite", Replayable),
        ("x-openai-internal-codex-residency", CredentialOwned),
        ("traceparent", CredentialOwned),
        ("tracestate", CredentialOwned),
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

pub(crate) fn request(context: ProviderRequestContext<'_>) -> Result<HeaderMap, ProviderError> {
    let mut headers = HeaderMap::new();
    super::identity::apply_data_defaults(&mut headers);
    if context.ingress_dialect == context.upstream_operation.dialect() {
        headers.extend(project_request(
            context.client_headers,
            &REQUEST_HEADERS,
            &[],
            context.allow_credential_bound,
            context.allow_turn_state,
        ));
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
    context: ProviderRequestContext<'_>,
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
    use crate::api::ProviderRequestContext;

    const OWNED_HEADERS: &[&str] = &[
        "x-oai-attestation",
        "x-client-request-id",
        "session-id",
        "thread-id",
        "x-codex-installation-id",
        "x-codex-window-id",
        "x-codex-turn-metadata",
        "x-codex-parent-thread-id",
        "x-openai-subagent",
        "x-openai-internal-codex-residency",
        "traceparent",
        "tracestate",
    ];

    #[test]
    fn credential_owned_codex_headers_are_not_replayed_after_a_switch() {
        let mut client = HeaderMap::new();
        for name in OWNED_HEADERS {
            client.insert(*name, HeaderValue::from_static("owned"));
        }
        client.insert("x-codex-turn-state", HeaderValue::from_static("turn-state"));
        client.insert("openai-beta", HeaderValue::from_static("responses=v1"));
        let context = ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "gpt",
            client_headers: &client,
            oauth: true,
            allow_credential_bound: false,
            allow_turn_state: false,
        };
        let projected = request(context).expect("headers");
        for name in OWNED_HEADERS {
            assert!(!projected.contains_key(*name), "unexpected {name}");
        }
        assert!(!projected.contains_key("x-codex-turn-state"));
        assert_eq!(projected["openai-beta"], "responses=v1");
        assert_eq!(projected["originator"], "codex_cli_rs");

        let owner = request(ProviderRequestContext {
            allow_credential_bound: true,
            allow_turn_state: true,
            ..context
        })
        .expect("owner headers");
        for name in OWNED_HEADERS {
            assert_eq!(owner[*name], "owned", "missing {name}");
        }
        assert_eq!(owner["x-codex-turn-state"], "turn-state");
    }

    #[test]
    fn codex_turn_state_requires_binding_after_credential_owner_matches() {
        let mut client = HeaderMap::new();
        client.insert("session-id", HeaderValue::from_static("session"));
        client.insert("x-codex-turn-state", HeaderValue::from_static("turn"));
        let context = ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "gpt",
            client_headers: &client,
            oauth: true,
            allow_credential_bound: true,
            allow_turn_state: false,
        };

        let unbound = request(context).expect("unbound headers");
        assert_eq!(unbound["session-id"], "session");
        assert!(!unbound.contains_key("x-codex-turn-state"));

        let bound = request(ProviderRequestContext {
            allow_turn_state: true,
            ..context
        })
        .expect("bound headers");
        assert_eq!(bound["x-codex-turn-state"], "turn");
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
