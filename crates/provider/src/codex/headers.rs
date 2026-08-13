use std::sync::LazyLock;

use any2api_domain::RequestBodyEncoding;
use http::{HeaderMap, HeaderName};

use crate::{
    ProviderError,
    api::ProviderRequestContext,
    header_policy::{ordered_names, project},
    request_header_policy::{
        RequestHeaderOwnership::{BoundTurnState, CredentialOwned, Replayable, SessionScoped},
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
        ("x-client-request-id", SessionScoped),
        ("session-id", SessionScoped),
        ("thread-id", SessionScoped),
        ("x-codex-installation-id", SessionScoped),
        ("x-codex-window-id", SessionScoped),
        ("x-codex-turn-metadata", SessionScoped),
        ("x-codex-parent-thread-id", SessionScoped),
        ("x-codex-beta-features", Replayable),
        ("x-openai-subagent", SessionScoped),
        ("x-openai-memgen-request", Replayable),
        ("x-responsesapi-include-timing-metrics", Replayable),
        ("x-openai-internal-codex-responses-lite", Replayable),
        ("x-openai-internal-codex-residency", SessionScoped),
        ("traceparent", SessionScoped),
        ("tracestate", SessionScoped),
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
            context.allow_session_replay,
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
    _context: ProviderRequestContext<'_>,
    _encoding: RequestBodyEncoding,
) -> bool {
    // Request-body compression is disabled: identity bodies keep the wire
    // bytes credential-invariant, which upstream prompt caching depends on.
    false
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolDialect, ProtocolOperation, RequestBodyEncoding};
    use http::{HeaderMap, HeaderValue};

    use super::{request, response, supports_encoding};
    use crate::api::ProviderRequestContext;

    const OWNED_HEADERS: &[&str] = &["x-oai-attestation"];
    const REPLAYABLE_HEADERS: &[(&str, &str)] = &[
        ("openai-beta", "responses=v1"),
        ("x-codex-beta-features", "remote_compaction_v2"),
        ("x-openai-internal-codex-responses-lite", "true"),
        ("x-client-request-id", "client-request"),
        ("session-id", "client-session"),
        ("thread-id", "client-thread"),
        ("x-codex-installation-id", "client-install"),
        ("x-codex-window-id", "client-window"),
        ("x-codex-turn-metadata", "client-turn-metadata"),
        ("x-codex-parent-thread-id", "client-parent-thread"),
        ("x-openai-subagent", "client-subagent"),
        ("x-openai-internal-codex-residency", "client-residency"),
        (
            "traceparent",
            "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01",
        ),
        ("tracestate", "client-tracestate"),
    ];

    #[test]
    fn credential_owned_codex_headers_are_not_replayed_after_a_switch() {
        let mut client = HeaderMap::new();
        for name in OWNED_HEADERS {
            client.insert(*name, HeaderValue::from_static("owned"));
        }
        client.insert("x-codex-turn-state", HeaderValue::from_static("turn-state"));
        for (name, value) in REPLAYABLE_HEADERS {
            client.insert(*name, HeaderValue::from_static(value));
        }
        let context = ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "gpt",
            client_headers: &client,
            oauth: true,
            allow_credential_bound: false,
            allow_session_replay: true,
            allow_turn_state: false,
        };
        let projected = request(context).expect("headers");
        for name in OWNED_HEADERS {
            assert!(!projected.contains_key(*name), "unexpected {name}");
        }
        assert!(!projected.contains_key("x-codex-turn-state"));
        for (name, value) in REPLAYABLE_HEADERS {
            assert_eq!(projected[*name], *value, "missing {name}");
        }
        assert_eq!(projected["originator"], "codex_cli_rs");

        let owner = request(ProviderRequestContext {
            allow_credential_bound: true,
            allow_session_replay: true,
            allow_turn_state: true,
            ..context
        })
        .expect("owner headers");
        for name in OWNED_HEADERS {
            assert_eq!(owner[*name], "owned", "missing {name}");
        }
        for (name, value) in REPLAYABLE_HEADERS {
            assert_eq!(owner[*name], *value, "missing {name}");
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
            allow_session_replay: true,
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

    #[test]
    fn request_body_compression_is_disabled_for_cache_stable_wire_bytes() {
        let client = HeaderMap::new();
        let oauth_responses = ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "gpt",
            client_headers: &client,
            oauth: true,
            allow_credential_bound: true,
            allow_session_replay: true,
            allow_turn_state: false,
        };

        assert!(!supports_encoding(
            oauth_responses,
            RequestBodyEncoding::Zstd
        ));
        assert!(!supports_encoding(
            oauth_responses,
            RequestBodyEncoding::Identity
        ));
    }
}
