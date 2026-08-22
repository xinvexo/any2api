use std::sync::Arc;

use any2api_domain::{ProtocolDialect, ProtocolOperation, ProviderBaseUrl, ProviderKind};
use http::{HeaderMap, HeaderValue};

use any2api_provider::{
    ClaudeDriver, CodexDriver, GrokDriver, KimiDriver, OpenAiDriver,
    api::{OfficialClientVersion, ProviderDriver, ProviderRegistry, ProviderRequestContext},
};

#[derive(Clone, Copy)]
enum Projection {
    CredentialOwner,
    CredentialSwitched,
    StickyCredentialSwitched,
    CrossDialect,
}

#[test]
fn registered_provider_operation_header_contracts_match_the_golden_profiles() {
    let registry = provider_registry();
    assert_eq!(registry.iter().count(), ProviderKind::ALL.len());

    for kind in ProviderKind::ALL {
        let driver = registry.get(kind).expect("registered Provider driver");
        let operations = supported_operations(driver.as_ref());
        assert!(!operations.is_empty(), "{kind:?} has no operation contract");
        for operation in operations {
            assert_projection_contract(driver.as_ref(), operation, false);
            if driver.descriptor().supports_oauth_operation(operation) {
                assert_projection_contract(driver.as_ref(), operation, true);
            }
        }
    }
}

fn assert_projection_contract(
    driver: &dyn ProviderDriver,
    operation: ProtocolOperation,
    oauth: bool,
) {
    let client_headers = client_headers();
    for projection in [
        Projection::CredentialOwner,
        Projection::CredentialSwitched,
        Projection::StickyCredentialSwitched,
        Projection::CrossDialect,
    ] {
        let same_dialect = !matches!(projection, Projection::CrossDialect);
        let context = ProviderRequestContext {
            ingress_dialect: if same_dialect {
                operation.dialect()
            } else {
                different_dialect(operation.dialect())
            },
            upstream_operation: operation,
            upstream_model: "model-contract",
            client_headers: &client_headers,
            oauth,
            allow_credential_bound: matches!(projection, Projection::CredentialOwner),
            allow_turn_state: matches!(projection, Projection::CredentialOwner),
            allow_session_replay: !matches!(projection, Projection::StickyCredentialSwitched),
        };
        let actual = driver
            .prepare_request_headers(context)
            .expect("Provider request Header contract");
        assert_eq!(
            canonical_headers(&actual),
            golden(driver.kind(), oauth, projection),
            "{:?} {operation:?} oauth={oauth} projection={}",
            driver.kind(),
            projection_name(projection),
        );
    }
}

fn provider_registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    for driver in [
        Arc::new(OpenAiDriver::new()) as Arc<dyn ProviderDriver>,
        Arc::new(CodexDriver::new_with_official_client_version(version(
            "0.145.0",
        ))) as Arc<dyn ProviderDriver>,
        Arc::new(ClaudeDriver::new_with_official_client_version(version(
            "2.1.220",
        ))),
        Arc::new(GrokDriver::new_with_official_client_version(version(
            "0.2.112",
        ))),
        Arc::new(KimiDriver::new()),
    ] {
        registry.register(driver).expect("unique Provider driver");
    }
    registry
}

fn version(value: &str) -> OfficialClientVersion {
    OfficialClientVersion::new(value).expect("test client version")
}

fn supported_operations(driver: &dyn ProviderDriver) -> Vec<ProtocolOperation> {
    let base_url = ProviderBaseUrl::parse("https://api.example.com/v1").expect("base URL");
    ProtocolOperation::ALL
        .into_iter()
        .filter(|operation| driver.endpoint_plan(&base_url, *operation).is_ok())
        .collect()
}

fn client_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("authorization", "Bearer gateway-secret"),
        ("cookie", "session=secret"),
        ("openai-beta", "client-beta"),
        ("originator", "client-origin"),
        ("session-id", "client-session"),
        (
            "traceparent",
            "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01",
        ),
        ("user-agent", "client-agent/9"),
        ("x-client-request-id", "client-request"),
        ("x-codex-turn-state", "client-turn"),
        ("x-oai-attestation", "client-attestation"),
        ("anthropic-beta", "client-anthropic-beta"),
        ("anthropic-version", "2099-01-01"),
        ("x-app", "client-app"),
        ("x-claude-code-session-id", "client-claude-session"),
        ("x-stainless-retry-count", "7"),
        ("x-grok-client-identifier", "client-grok"),
        ("x-grok-client-mode", "client-mode"),
        ("x-grok-client-surface", "terminal"),
        ("x-grok-client-version", "9.9"),
        ("x-grok-conv-id", "client-conv"),
        ("x-grok-req-id", "client-grok-request"),
    ] {
        headers.insert(
            http::HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    headers
}

fn golden(kind: ProviderKind, oauth: bool, projection: Projection) -> String {
    let value = match (kind, oauth, projection) {
        (ProviderKind::OpenAi, false, _) => "",
        (ProviderKind::OpenAi, true, _) => panic!("OpenAI has no OAuth operation"),
        (ProviderKind::Codex, _, Projection::CredentialOwner) => CODEX_OWNER,
        (ProviderKind::Codex, _, Projection::CredentialSwitched) => CODEX_SWITCHED,
        (ProviderKind::Codex, _, Projection::StickyCredentialSwitched) => CODEX_SWITCHED_STICKY,
        (ProviderKind::Codex, _, Projection::CrossDialect) => CODEX_CROSS,
        (ProviderKind::Claude, _, Projection::CredentialOwner) => CLAUDE_OWNER,
        (ProviderKind::Claude, _, Projection::CredentialSwitched) => CLAUDE_SWITCHED,
        (ProviderKind::Claude, _, Projection::StickyCredentialSwitched) => CLAUDE_SWITCHED_STICKY,
        (ProviderKind::Claude, _, Projection::CrossDialect) => CLAUDE_CROSS,
        (ProviderKind::Grok, false, Projection::CredentialOwner) => GROK_OWNER,
        (ProviderKind::Grok, false, Projection::CredentialSwitched) => GROK_SWITCHED,
        (ProviderKind::Grok, false, Projection::StickyCredentialSwitched) => GROK_SWITCHED_STICKY,
        (ProviderKind::Grok, false, Projection::CrossDialect) => GROK_CROSS,
        (ProviderKind::Grok, true, Projection::CredentialOwner) => GROK_OAUTH_OWNER,
        (ProviderKind::Grok, true, Projection::CredentialSwitched) => GROK_OAUTH_SWITCHED,
        (ProviderKind::Grok, true, Projection::StickyCredentialSwitched) => {
            GROK_OAUTH_SWITCHED_STICKY
        }
        (ProviderKind::Grok, true, Projection::CrossDialect) => GROK_OAUTH_CROSS,
        (ProviderKind::Kimi, false, _) => "",
        (ProviderKind::Kimi, true, _) => panic!("Kimi has no OAuth operation"),
    };
    value.replace("{grok-user-agent}", &grok_user_agent())
}

fn canonical_headers(headers: &HeaderMap) -> String {
    let mut lines = headers
        .iter()
        .map(|(name, value)| {
            format!(
                "{}: {}",
                name.as_str(),
                value.to_str().expect("golden Header is visible ASCII")
            )
        })
        .collect::<Vec<_>>();
    lines.sort_unstable();
    lines.join("\n")
}

fn grok_user_agent() -> String {
    format!(
        "grok-shell/0.2.112 ({}; {})",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn different_dialect(dialect: ProtocolDialect) -> ProtocolDialect {
    match dialect {
        ProtocolDialect::OpenAiResponses => ProtocolDialect::AnthropicMessages,
        _ => ProtocolDialect::OpenAiResponses,
    }
}

fn projection_name(projection: Projection) -> &'static str {
    match projection {
        Projection::CredentialOwner => "owner",
        Projection::CredentialSwitched => "switched",
        Projection::StickyCredentialSwitched => "sticky-switched",
        Projection::CrossDialect => "cross-dialect",
    }
}

const CODEX_OWNER: &str = "openai-beta: client-beta\noriginator: client-origin\nsession-id: client-session\ntraceparent: 00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01\nuser-agent: client-agent/9\nx-client-request-id: client-request\nx-codex-turn-state: client-turn\nx-oai-attestation: client-attestation";
const CODEX_SWITCHED: &str = "openai-beta: client-beta\noriginator: client-origin\nsession-id: client-session\ntraceparent: 00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01\nuser-agent: client-agent/9\nx-client-request-id: client-request";
const CODEX_SWITCHED_STICKY: &str =
    "openai-beta: client-beta\noriginator: client-origin\nuser-agent: client-agent/9";
const CODEX_CROSS: &str = "originator: codex_cli_rs\nuser-agent: codex_cli_rs/0.145.0";

const CLAUDE_OWNER: &str = "anthropic-beta: client-anthropic-beta\nanthropic-version: 2099-01-01\ntraceparent: 00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01\nuser-agent: client-agent/9\nx-app: client-app\nx-claude-code-session-id: client-claude-session\nx-client-request-id: client-request\nx-stainless-retry-count: 7";
const CLAUDE_SWITCHED: &str = "anthropic-beta: client-anthropic-beta\nanthropic-version: 2099-01-01\ntraceparent: 00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01\nuser-agent: client-agent/9\nx-app: client-app\nx-claude-code-session-id: client-claude-session\nx-client-request-id: client-request\nx-stainless-retry-count: 7";
const CLAUDE_SWITCHED_STICKY: &str = "anthropic-beta: client-anthropic-beta\nanthropic-version: 2099-01-01\nuser-agent: client-agent/9\nx-app: client-app";
const CLAUDE_CROSS: &str =
    "anthropic-version: 2023-06-01\nuser-agent: claude-code/2.1.220\nx-app: cli";

const GROK_OWNER: &str = "traceparent: 00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01\nuser-agent: client-agent/9\nx-grok-client-identifier: client-grok\nx-grok-client-mode: client-mode\nx-grok-client-surface: terminal\nx-grok-client-version: 9.9\nx-grok-conv-id: client-conv\nx-grok-req-id: client-grok-request";
const GROK_SWITCHED: &str = "traceparent: 00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01\nuser-agent: client-agent/9\nx-grok-client-identifier: client-grok\nx-grok-client-mode: client-mode\nx-grok-client-surface: terminal\nx-grok-client-version: 9.9\nx-grok-conv-id: client-conv\nx-grok-req-id: client-grok-request";
const GROK_SWITCHED_STICKY: &str = "user-agent: client-agent/9\nx-grok-client-identifier: client-grok\nx-grok-client-mode: client-mode\nx-grok-client-surface: terminal\nx-grok-client-version: 9.9";
const GROK_CROSS: &str = "user-agent: {grok-user-agent}\nx-grok-client-identifier: grok-shell\nx-grok-client-version: 0.2.112";
const GROK_OAUTH_OWNER: &str = "traceparent: 00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01\nuser-agent: client-agent/9\nx-authenticateresponse: authenticate-response\nx-grok-client-identifier: client-grok\nx-grok-client-mode: client-mode\nx-grok-client-surface: terminal\nx-grok-client-version: 9.9\nx-grok-conv-id: client-conv\nx-grok-model-override: model-contract\nx-grok-req-id: client-grok-request\nx-xai-token-auth: xai-grok-cli";
const GROK_OAUTH_SWITCHED: &str = "traceparent: 00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-bbbbbbbbbbbbbbbb-01\nuser-agent: client-agent/9\nx-authenticateresponse: authenticate-response\nx-grok-client-identifier: client-grok\nx-grok-client-mode: client-mode\nx-grok-client-surface: terminal\nx-grok-client-version: 9.9\nx-grok-conv-id: client-conv\nx-grok-model-override: model-contract\nx-grok-req-id: client-grok-request\nx-xai-token-auth: xai-grok-cli";
const GROK_OAUTH_SWITCHED_STICKY: &str = "user-agent: client-agent/9\nx-authenticateresponse: authenticate-response\nx-grok-client-identifier: client-grok\nx-grok-client-mode: client-mode\nx-grok-client-surface: terminal\nx-grok-client-version: 9.9\nx-grok-model-override: model-contract\nx-xai-token-auth: xai-grok-cli";
const GROK_OAUTH_CROSS: &str = "user-agent: {grok-user-agent}\nx-authenticateresponse: authenticate-response\nx-grok-client-identifier: grok-shell\nx-grok-client-mode: interactive\nx-grok-client-version: 0.2.112\nx-grok-model-override: model-contract\nx-xai-token-auth: xai-grok-cli";
