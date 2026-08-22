use std::sync::LazyLock;

use http::{HeaderMap, HeaderName};

use crate::{
    ProviderError,
    api::{OfficialClientVersion, ProviderRequestContext},
    header_policy::{ordered_names, project},
    request_header_policy::{
        RequestHeaderOwnership::{CredentialOwned, Replayable, SessionScoped},
        RequestHeaderPrefixRule, RequestHeaderRule, project_request, request_header_rules,
    },
};

static REQUEST_HEADERS: LazyLock<Vec<RequestHeaderRule>> = LazyLock::new(|| {
    request_header_rules(&[
        ("anthropic-version", Replayable),
        ("anthropic-beta", Replayable),
        ("anthropic-mcp-client-capabilities", Replayable),
        ("user-agent", Replayable),
        ("x-app", Replayable),
        ("x-client-request-id", SessionScoped),
        ("x-claude-code-session-id", SessionScoped),
        ("anthropic-usage-limit", CredentialOwned),
        ("anthropic-dangerous-direct-browser-access", Replayable),
        ("anthropic-client-platform", Replayable),
        ("x-anthropic-additional-protection", CredentialOwned),
        ("x-claude-remote-container-id", SessionScoped),
        ("x-claude-remote-session-id", SessionScoped),
        ("x-claude-code-agent-id", SessionScoped),
        ("x-claude-code-parent-agent-id", SessionScoped),
        ("traceparent", SessionScoped),
        ("tracestate", SessionScoped),
    ])
});

static REQUEST_HEADER_PREFIXES: [RequestHeaderPrefixRule; 1] =
    [RequestHeaderPrefixRule::new("x-stainless-", SessionScoped)];

static RESPONSE_HEADERS: LazyLock<Vec<HeaderName>> = LazyLock::new(|| {
    ordered_names(&[
        "content-encoding",
        "content-type",
        "request-id",
        "x-request-id",
        "retry-after",
        "retry-after-ms",
        "x-should-retry",
        "anthropic-usage-limit",
        "cf-ray",
    ])
});

pub(crate) fn request(
    context: ProviderRequestContext<'_>,
    version: &OfficialClientVersion,
) -> Result<HeaderMap, ProviderError> {
    let mut headers = HeaderMap::new();
    super::identity::apply_data_defaults(&mut headers, version);
    if context.ingress_dialect == context.upstream_operation.dialect() {
        headers.extend(project_request(
            context.client_headers,
            &REQUEST_HEADERS,
            &REQUEST_HEADER_PREFIXES,
            context.allow_credential_bound,
            context.allow_turn_state,
            context.allow_session_replay,
        ));
    }
    Ok(headers)
}

pub(crate) fn response(upstream: &HeaderMap) -> HeaderMap {
    project(upstream, RESPONSE_HEADERS.iter(), &["anthropic-ratelimit-"])
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolDialect, ProtocolOperation};
    use http::{HeaderMap, HeaderValue};

    use super::request;
    use crate::api::{OfficialClientVersion, ProviderRequestContext};

    fn version() -> OfficialClientVersion {
        OfficialClientVersion::new("9.8.7").expect("version")
    }

    const OWNED_HEADERS: &[&str] = &["anthropic-usage-limit", "x-anthropic-additional-protection"];
    const REPLAYABLE_SESSION_HEADERS: &[&str] = &[
        "x-client-request-id",
        "x-claude-code-session-id",
        "x-claude-remote-container-id",
        "x-claude-remote-session-id",
        "x-claude-code-agent-id",
        "x-claude-code-parent-agent-id",
        "traceparent",
        "tracestate",
        "x-stainless-retry-count",
    ];

    #[test]
    fn claude_session_headers_survive_a_credential_switch() {
        let mut client = HeaderMap::new();
        for name in OWNED_HEADERS {
            client.insert(*name, HeaderValue::from_static("owned"));
        }
        for name in REPLAYABLE_SESSION_HEADERS {
            client.insert(*name, HeaderValue::from_static("session"));
        }
        client.insert("anthropic-beta", HeaderValue::from_static("feature"));
        let context = ProviderRequestContext {
            ingress_dialect: ProtocolDialect::AnthropicMessages,
            upstream_operation: ProtocolOperation::Messages,
            upstream_model: "claude",
            client_headers: &client,
            oauth: false,
            allow_credential_bound: false,
            allow_session_replay: true,
            allow_turn_state: false,
        };

        let switched = request(context, &version()).expect("switched headers");
        for name in OWNED_HEADERS {
            assert!(!switched.contains_key(*name), "unexpected {name}");
        }
        for name in REPLAYABLE_SESSION_HEADERS {
            assert_eq!(switched[*name], "session", "missing {name}");
        }
        assert_eq!(switched["anthropic-beta"], "feature");
        assert_eq!(switched["x-app"], "cli");

        let owner = request(
            ProviderRequestContext {
                allow_credential_bound: true,
                allow_session_replay: true,
                ..context
            },
            &version(),
        )
        .expect("owner headers");
        for name in OWNED_HEADERS {
            assert_eq!(owner[*name], "owned", "missing {name}");
        }
    }
}
