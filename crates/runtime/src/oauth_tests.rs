use std::time::Instant;

use any2api_domain::ProviderKind;
use any2api_provider::api::{OAuthTokenMaterial, serialize_file};

use super::{
    callback,
    session::{OAuthSession, OAuthSessionStore},
};

#[test]
fn oauth_session_is_consumed_once() {
    let prepared = OAuthSession::prepare(
        ProviderKind::Codex,
        "http://localhost:1455/auth/callback",
        Instant::now(),
    )
    .expect("session should be prepared");
    let id = prepared.id.clone();
    let mut store = OAuthSessionStore::default();
    store
        .insert(id.clone(), prepared.session, Instant::now())
        .expect("session should be inserted");

    store
        .take(&id, Instant::now())
        .expect("first exchange should consume the session");
    assert!(store.take(&id, Instant::now()).is_err());
}

#[test]
fn oauth_callback_rejects_state_and_redirect_mismatches() {
    let redirect = "http://localhost:1455/auth/callback";
    let state_error = callback::parse(
        "http://localhost:1455/auth/callback?code=abc&state=wrong",
        redirect,
        "expected",
    )
    .expect_err("state mismatch must be rejected");
    assert!(matches!(
        state_error,
        super::error::OAuthError::StateMismatch
    ));

    let redirect_error = callback::parse(
        "http://localhost:1455/other?code=abc&state=expected",
        redirect,
        "expected",
    )
    .expect_err("redirect target mismatch must be rejected");
    assert!(matches!(
        redirect_error,
        super::error::OAuthError::InvalidCallback
    ));
}

#[test]
fn oauth_files_use_provider_specific_shapes() {
    let codex = OAuthTokenMaterial::new(
        ProviderKind::Codex,
        "access-secret".into(),
        Some("refresh-secret".into()),
        Some("id-secret".into()),
        Some(1_700_000_000),
        Some("account-123".into()),
        Some("person@example.com".into()),
    )
    .expect("Codex token");
    let codex_file = String::from_utf8(
        serialize_file(&codex, "2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z").expect("Codex file"),
    )
    .expect("UTF-8 file");
    assert!(codex_file.contains("\"account_id\": \"account-123\""));
    assert!(codex_file.contains("\"type\": \"codex\""));

    let claude = OAuthTokenMaterial::new(
        ProviderKind::Claude,
        "claude-access-secret".into(),
        Some("claude-refresh-secret".into()),
        None,
        None,
        None,
        Some("claude@example.com".into()),
    )
    .expect("Claude token");
    let claude_file = String::from_utf8(
        serialize_file(&claude, "2026-01-01T00:00:00Z", "").expect("Claude file"),
    )
    .expect("UTF-8 file");
    assert!(!claude_file.contains("account_id"));
    assert!(claude_file.contains("\"type\": \"claude\""));
}

#[test]
fn oauth_debug_output_redacts_token_material() {
    let token = OAuthTokenMaterial::new(
        ProviderKind::Codex,
        "access-secret".into(),
        Some("refresh-secret".into()),
        Some("id-secret".into()),
        None,
        None,
        None,
    )
    .expect("token");
    let debug = format!("{token:?}");
    assert!(!debug.contains("access-secret"));
    assert!(!debug.contains("refresh-secret"));
    assert!(!debug.contains("id-secret"));
    assert!(debug.contains("REDACTED"));
}
