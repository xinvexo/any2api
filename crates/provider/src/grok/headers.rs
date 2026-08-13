use std::sync::LazyLock;

use any2api_domain::ProviderKind;
use http::{HeaderMap, HeaderName, HeaderValue};

use crate::{
    ProviderError,
    api::ProviderRequestContext,
    header_policy::{ordered_names, project},
    request_header_policy::{
        RequestHeaderOwnership::{Replayable, SessionScoped},
        RequestHeaderRule, project_request, request_header_rules,
    },
};

static REQUEST_HEADERS: LazyLock<Vec<RequestHeaderRule>> = LazyLock::new(|| {
    request_header_rules(&[
        ("x-grok-conv-id", SessionScoped),
        ("x-grok-req-id", SessionScoped),
        ("x-grok-session-id", SessionScoped),
        ("x-grok-agent-id", SessionScoped),
        ("x-grok-turn-id", SessionScoped),
        ("user-agent", Replayable),
        ("x-grok-client-mode", Replayable),
        ("x-grok-client-version", Replayable),
        ("x-grok-client-identifier", Replayable),
        ("x-grok-client-surface", Replayable),
        ("traceparent", SessionScoped),
        ("tracestate", SessionScoped),
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
    super::identity::apply_data_defaults(&mut headers, context.oauth);
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

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolDialect, ProtocolOperation};
    use http::{HeaderMap, HeaderValue};

    use super::request;
    use crate::api::ProviderRequestContext;

    const SESSION_HEADERS: &[&str] = &[
        "x-grok-conv-id",
        "x-grok-req-id",
        "x-grok-session-id",
        "x-grok-agent-id",
        "x-grok-turn-id",
        "traceparent",
        "tracestate",
    ];

    #[test]
    fn grok_session_headers_survive_a_credential_switch() {
        let mut client = HeaderMap::new();
        for name in SESSION_HEADERS {
            client.insert(*name, HeaderValue::from_static("session"));
        }
        client.insert(
            "x-grok-client-surface",
            HeaderValue::from_static("terminal"),
        );
        let context = ProviderRequestContext {
            ingress_dialect: ProtocolDialect::OpenAiResponses,
            upstream_operation: ProtocolOperation::Responses,
            upstream_model: "grok",
            client_headers: &client,
            oauth: false,
            allow_credential_bound: false,
            allow_session_replay: true,
            allow_turn_state: false,
        };

        let switched = request(context).expect("switched headers");
        for name in SESSION_HEADERS {
            assert_eq!(switched[*name], "session", "missing {name}");
        }
        assert_eq!(switched["x-grok-client-surface"], "terminal");
        assert_eq!(switched["x-grok-client-identifier"], "grok-shell");
    }
}
