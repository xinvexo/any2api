use std::{str::FromStr, time::Duration};

use any2api_domain::{
    ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind, RequestBodyEncoding,
};
use any2api_protocol::api::{IngressRequest, ProtocolRegistry};
use any2api_provider::api::{
    OAuthTokenMaterial, ProviderDriver, ProviderRequestContext, ProviderSecret,
};
use any2api_transport::api::{
    EndpointNetworkPolicy, TransportIsolationKey, TransportRequest, TransportTrafficClass,
};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Uri, header};

use super::types::{Surface, SurfaceCase};

pub(super) enum DataCredential<'a> {
    ApiKey,
    OAuth(&'a OAuthTokenMaterial),
}

pub(super) struct DataCaseSpec<'a> {
    pub(super) ingress: ProtocolDialect,
    pub(super) upstream: ProtocolDialect,
    pub(super) operation: ProtocolOperation,
    pub(super) base_url: &'a ProviderBaseUrl,
    pub(super) credential: DataCredential<'a>,
    pub(super) surface: Surface,
}

pub(super) async fn data_case(
    protocols: &ProtocolRegistry,
    driver: &dyn ProviderDriver,
    spec: DataCaseSpec<'_>,
) -> SurfaceCase {
    let DataCaseSpec {
        ingress,
        upstream,
        operation,
        base_url,
        credential,
        surface,
    } = spec;
    let adapter = protocols.get(ingress).expect("ingress protocol adapter");
    let (headers, body) = ingress_fixture(driver.kind(), operation);
    let decoded = adapter
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: Uri::from_static("/"),
            headers,
            body,
            operation,
        })
        .await
        .expect("surface ingress request");
    assert_eq!(decoded.body_encoding, RequestBodyEncoding::Identity);

    let mut exchange = protocols
        .exchange(ingress, upstream, operation)
        .expect("surface protocol exchange");
    let prepared = exchange
        .prepare_request_with_target_profile(
            &decoded,
            "fixture-upstream-model",
            driver.protocol_target_profile(upstream, "fixture-upstream-model"),
            None,
        )
        .expect("surface upstream request");
    let endpoint = driver
        .endpoint_plan(base_url, prepared.upstream_operation)
        .expect("surface Provider endpoint");
    let context = ProviderRequestContext {
        ingress_dialect: ingress,
        upstream_operation: prepared.upstream_operation,
        upstream_model: "fixture-upstream-model",
        client_headers: &decoded.client_headers,
        oauth: matches!(credential, DataCredential::OAuth(_)),
        allow_credential_bound: true,
        allow_session_replay: true,
        allow_turn_state: true,
    };
    let body = driver
        .prepare_request_body(context, prepared.request.body)
        .expect("surface Provider body");
    let mut headers = driver
        .prepare_request_headers(context)
        .expect("surface Provider headers");
    headers.extend(prepared.request.headers);
    headers.remove(header::CONTENT_ENCODING);
    let (auth_class, credential_headers) = match credential {
        DataCredential::ApiKey => (
            "api_key",
            driver
                .credential_headers(
                    base_url,
                    &ProviderSecret::new(format!("fixture-{}-api-key", driver.kind().as_str())),
                )
                .expect("surface API Key headers"),
        ),
        DataCredential::OAuth(token) => (
            "oauth_access_token",
            driver
                .oauth_credential_headers(token, &headers)
                .expect("surface OAuth headers"),
        ),
    };
    headers.extend(credential_headers.headers);
    let name = match surface {
        Surface::DataDirect => format!(
            "data.{}.{}.direct.{}",
            driver.kind().as_str(),
            auth_class,
            operation.as_str()
        ),
        Surface::DataBridge => format!(
            "data.{}.{}.bridge.{}_to_{}.{}",
            driver.kind().as_str(),
            auth_class,
            ingress.as_str(),
            upstream.as_str(),
            operation.as_str()
        ),
        Surface::OAuthToken | Surface::OAuthQuota => unreachable!("data surface"),
    };
    let target = endpoint.url.to_string();
    SurfaceCase {
        name,
        provider: driver.kind(),
        surface,
        auth_class,
        target,
        request: TransportRequest {
            method: prepared.request.method,
            uri: Uri::from_str(endpoint.url.as_str()).expect("surface endpoint URI"),
            headers,
            body,
            isolation: TransportIsolationKey::ephemeral(TransportTrafficClass::DataPlane),
            network_policy: EndpointNetworkPolicy::new(),
            read_timeout: Duration::from_secs(15),
        },
    }
}

pub(super) fn api_key_base_url(kind: ProviderKind) -> ProviderBaseUrl {
    let value = match kind {
        ProviderKind::OpenAi => "https://api.openai.com/v1",
        ProviderKind::Codex => "https://api.openai.com/v1",
        ProviderKind::Claude => "https://api.anthropic.com",
        ProviderKind::Grok => "https://api.x.ai/v1",
        ProviderKind::Kimi => "https://api.moonshot.cn/v1",
    };
    ProviderBaseUrl::parse(value).expect("fixture Provider base URL")
}

fn ingress_fixture(provider: ProviderKind, operation: ProtocolOperation) -> (HeaderMap, Bytes) {
    let mut headers = client_identity_headers(provider);
    let body = match operation {
        ProtocolOperation::Responses => {
            Bytes::from_static(br#"{"model":"public-model","input":"hello","stream":false}"#)
        }
        ProtocolOperation::ResponsesCompact => Bytes::from_static(
            br#"{"model":"public-model","input":[{"role":"user","content":"compact me"}]}"#,
        ),
        ProtocolOperation::AlphaSearch => Bytes::from_static(
            br#"{"id":"fixture-session","model":"public-model","input":[{"type":"message","role":"user","content":[{"type":"input_text","text":"find the moon"}]}],"commands":{"search_query":[{"q":"moon phase today","recency":7}]},"settings":{"allowed_callers":["direct"],"external_web_access":true},"max_output_tokens":2500}"#,
        ),
        ProtocolOperation::ChatCompletions => Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"hello"}],"stream":false}"#,
        ),
        ProtocolOperation::ImagesGenerations => Bytes::from_static(
            br#"{"model":"public-model","prompt":"draw a moon","stream":false,"response_format":"url"}"#,
        ),
        ProtocolOperation::ImagesEdits => {
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("multipart/form-data; boundary=fixture-boundary"),
            );
            return (
                headers,
                Bytes::from_static(
                    b"--fixture-boundary\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\npublic-model\r\n--fixture-boundary\r\nContent-Disposition: form-data; name=\"prompt\"\r\n\r\nedit the moon\r\n--fixture-boundary\r\nContent-Disposition: form-data; name=\"image\"; filename=\"moon.png\"\r\nContent-Type: image/png\r\n\r\nPNG-FIXTURE\r\n--fixture-boundary--\r\n",
                ),
            );
        }
        ProtocolOperation::Messages => Bytes::from_static(
            br#"{"model":"public-model","max_tokens":16,"messages":[{"role":"user","content":"hello"}],"stream":false}"#,
        ),
        ProtocolOperation::MessagesCountTokens => Bytes::from_static(
            br#"{"model":"public-model","messages":[{"role":"user","content":"count me"}]}"#,
        ),
    };
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    (headers, body)
}

fn client_identity_headers(provider: ProviderKind) -> HeaderMap {
    let mut headers = HeaderMap::new();
    match provider {
        ProviderKind::OpenAi => {}
        ProviderKind::Codex => {
            insert(&mut headers, "user-agent", "fixture-codex-client/9");
            insert(&mut headers, "originator", "fixture-origin");
            insert(&mut headers, "openai-beta", "fixture-beta");
            insert(&mut headers, "session-id", "fixture-session");
            insert(
                &mut headers,
                "traceparent",
                "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01",
            );
            insert(&mut headers, "x-client-request-id", "fixture-codex-request");
            insert(&mut headers, "x-codex-turn-state", "fixture-turn");
        }
        ProviderKind::Claude => {
            insert(&mut headers, "user-agent", "fixture-claude-client/9");
            insert(&mut headers, "anthropic-beta", "fixture-beta");
            insert(&mut headers, "anthropic-version", "2099-01-01");
            insert(&mut headers, "x-app", "fixture-app");
            insert(
                &mut headers,
                "x-claude-code-session-id",
                "fixture-claude-session",
            );
            insert(
                &mut headers,
                "x-client-request-id",
                "fixture-claude-request",
            );
            insert(&mut headers, "x-stainless-retry-count", "7");
        }
        ProviderKind::Grok => {
            insert(&mut headers, "user-agent", "fixture-grok-client/9");
            insert(
                &mut headers,
                "traceparent",
                "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01",
            );
            insert(&mut headers, "x-grok-client-identifier", "fixture-grok");
            insert(&mut headers, "x-grok-client-mode", "fixture-mode");
            insert(&mut headers, "x-grok-client-surface", "fixture-surface");
            insert(&mut headers, "x-grok-client-version", "9.9");
            insert(&mut headers, "x-grok-conv-id", "fixture-conversation");
            insert(&mut headers, "x-grok-req-id", "fixture-grok-request");
        }
        ProviderKind::Kimi => {
            insert(&mut headers, "user-agent", "fixture-kimi-client/9");
            insert(
                &mut headers,
                "traceparent",
                "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01",
            );
        }
    }
    headers
}

fn insert(headers: &mut HeaderMap, name: &'static str, value: &'static str) {
    headers.insert(name, HeaderValue::from_static(value));
}
