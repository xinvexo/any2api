use std::sync::Arc;

use any2api_domain::ProviderKind;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::json;

use crate::{
    ClaudeDriver, CodexDriver, GrokDriver,
    api::{OAuthImportParseError, ProviderRegistry, parse_oauth_import_document},
};

#[test]
fn imports_cliproxyapi_provider_documents() {
    let files = [
        json!({
            "type": "codex",
            "access_token": "codex-access",
            "refresh_token": "codex-refresh",
            "id_token": jwt(json!({
                "email": "codex@example.com",
                "https://api.openai.com/auth": {"chatgpt_account_id": "acct-codex"}
            })),
            "expired": "2030-01-01T00:00:00Z"
        }),
        json!({
            "type": "claude",
            "access_token": "claude-access",
            "refresh_token": "claude-refresh",
            "email": "claude@example.com"
        }),
        json!({
            "type": "xai",
            "auth_kind": "oauth",
            "access_token": "grok-access",
            "refresh_token": "grok-refresh",
            "sub": "acct-grok",
            "email": "grok@example.com"
        }),
    ];
    let registry = registry();
    let accounts = files
        .iter()
        .flat_map(|file| {
            parse_oauth_import_document(&registry, &serde_json::to_vec(file).expect("JSON"))
                .expect("CLIProxyAPI document")
        })
        .collect::<Vec<_>>();

    let mut accounts = accounts.into_iter();
    let (codex, _) = accounts.next().expect("Codex").into_parts();
    assert_eq!(codex.provider(), ProviderKind::Codex);
    assert_eq!(codex.access_token(), "codex-access");
    assert_eq!(codex.refresh_token(), Some("codex-refresh"));
    assert_eq!(codex.account_id(), Some("acct-codex"));
    assert_eq!(codex.email(), Some("codex@example.com"));
    assert_eq!(codex.expires_at(), Some(1_893_456_000));
    let (claude, _) = accounts.next().expect("Claude").into_parts();
    assert_eq!(claude.provider(), ProviderKind::Claude);
    assert_eq!(claude.access_token(), "claude-access");
    assert_eq!(claude.email(), Some("claude@example.com"));
    let (grok, _) = accounts.next().expect("Grok").into_parts();
    assert_eq!(grok.provider(), ProviderKind::Grok);
    assert_eq!(grok.access_token(), "grok-access");
    assert_eq!(grok.account_id(), Some("acct-grok"));
    assert!(accounts.next().is_none());
}

#[test]
fn imports_sub2api_accounts_envelope_across_registered_providers() {
    let body = serde_json::to_vec(&json!({
        "type": "sub2api-data",
        "accounts": [
            {
                "name": "OpenAI One",
                "platform": "openai",
                "type": "OAuth",
                "credentials": {
                    "access_token": "codex-access",
                    "refresh_token": "codex-refresh",
                    "chatgpt_account_id": "acct-codex",
                    "email": "codex@example.com"
                }
            },
            {
                "name": "Claude One",
                "platform": "anthropic",
                "type": "oauth",
                "credentials": {
                    "access_token": "claude-access",
                    "refresh_token": "claude-refresh",
                    "expires_at": "1893456000"
                }
            },
            {
                "name": "Grok One",
                "platform": "grok",
                "type": "oauth",
                "credentials": {
                    "access_token": "grok-access",
                    "refresh_token": "grok-refresh",
                    "sub": "acct-grok"
                }
            }
        ]
    }))
    .expect("JSON");

    let accounts = parse_oauth_import_document(&registry(), &body).expect("Sub2API envelope");
    let parts = accounts
        .into_iter()
        .map(|account| account.into_parts())
        .collect::<Vec<_>>();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].0.provider(), ProviderKind::Codex);
    assert_eq!(parts[0].1.as_deref(), Some("OpenAI One"));
    assert_eq!(parts[1].0.provider(), ProviderKind::Claude);
    assert_eq!(parts[1].0.expires_at(), Some(1_893_456_000));
    assert_eq!(parts[2].0.provider(), ProviderKind::Grok);
    assert_eq!(parts[2].0.account_id(), Some("acct-grok"));
}

#[test]
fn imports_codex_camel_case_session_arrays_and_jwt_fallbacks() {
    let access_token = jwt(json!({
        "exp": 1_900_000_000,
        "email": "session@example.com",
        "https://api.openai.com/auth": {"chatgpt_account_id": "acct-session"}
    }));
    let body = serde_json::to_vec(&json!([{
        "name": "Session One",
        "tokens": {
            "accessToken": access_token,
            "refreshToken": "session-refresh",
            "expiresAt": 1_900_000_000_000_i64
        },
        "user": {"email": "session@example.com"}
    }]))
    .expect("JSON");

    let mut accounts = parse_oauth_import_document(&registry(), &body).expect("Codex session");
    let (token, label) = accounts.pop().expect("account").into_parts();
    assert_eq!(token.provider(), ProviderKind::Codex);
    assert_eq!(token.refresh_token(), Some("session-refresh"));
    assert_eq!(token.expires_at(), Some(1_900_000_000));
    assert_eq!(token.account_id(), Some("acct-session"));
    assert_eq!(label.as_deref(), Some("Session One"));
}

#[test]
fn rejects_non_oauth_and_redacts_imported_secrets() {
    let error = parse_oauth_import_document(
        &registry(),
        br#"{"platform":"openai","type":"api_key","credentials":{"access_token":"secret"}}"#,
    )
    .expect_err("API Key account is not OAuth");
    assert_eq!(
        error,
        OAuthImportParseError::UnsupportedAccount { account_index: 1 }
    );

    let account = parse_oauth_import_document(
        &registry(),
        br#"{"type":"claude","access_token":"secret-access"}"#,
    )
    .expect("OAuth account")
    .pop()
    .expect("account");
    assert!(!format!("{account:?}").contains("secret-access"));
}

#[test]
fn does_not_guess_codex_from_an_untyped_generic_account_object() {
    let error = parse_oauth_import_document(
        &registry(),
        br#"{"access_token":"opaque-token","account":{"email_address":"person@example.com"}}"#,
    )
    .expect_err("generic OAuth material needs an explicit provider");

    assert_eq!(
        error,
        OAuthImportParseError::UnsupportedAccount { account_index: 1 }
    );
}

fn registry() -> ProviderRegistry {
    let mut registry = ProviderRegistry::new();
    registry
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex");
    registry
        .register(Arc::new(ClaudeDriver::new()))
        .expect("Claude");
    registry
        .register(Arc::new(GrokDriver::new()))
        .expect("Grok");
    registry
}

fn jwt(payload: serde_json::Value) -> String {
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).expect("JWT payload"));
    format!("header.{payload}.signature")
}
