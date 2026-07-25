use std::time::Instant;

use any2api_domain::ProviderKind;
use any2api_provider::api::{OAuthTokenMaterial, serialize_document};

use super::{
    callback,
    session::{AuthorizationCodeSession, DeviceCodeSession, OAuthSessionStore},
};

#[test]
fn oauth_session_is_consumed_once() {
    let prepared = AuthorizationCodeSession::prepare(
        ProviderKind::Codex,
        "http://localhost:1455/auth/callback",
        Instant::now(),
    )
    .expect("session should be prepared");
    let id = prepared.id.clone();
    let mut store = OAuthSessionStore::default();
    store
        .insert_authorization_code(id.clone(), prepared.session, Instant::now())
        .expect("session should be inserted");

    store
        .take_authorization_code(&id, Instant::now())
        .expect("first exchange should consume the session");
    assert!(store.take_authorization_code(&id, Instant::now()).is_err());
}

#[test]
fn device_session_is_rate_gated_and_keeps_flow_types_separate() {
    let now = Instant::now();
    let prepared = DeviceCodeSession::prepare(ProviderKind::Grok, "device-secret", 1_800, 5, now)
        .expect("device session should be prepared");
    let id = prepared.id.clone();
    let mut store = OAuthSessionStore::default();
    store
        .insert_device_code(id.clone(), prepared.session, now)
        .expect("device session should be inserted");

    assert!(store.take_authorization_code(&id, now).is_err());
    let mut session = store
        .take_device_code(&id, now)
        .expect("device poll should take the session");
    assert_eq!(session.retry_after(now).expect("active session"), None);
    assert_eq!(session.defer(now, false).expect("pending poll"), 5);
    store.restore_device_code(id.clone(), session);

    let mut session = store
        .take_device_code(&id, now)
        .expect("pending session should remain available");
    assert_eq!(session.retry_after(now).expect("active session"), Some(5));
    assert_eq!(session.defer(now, true).expect("slow-down poll"), 10);
    assert_eq!(session.device_code(), "device-secret");
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
fn oauth_documents_use_provider_specific_shapes() {
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
    let codex_document = String::from_utf8(
        serialize_document(&codex, "2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z")
            .expect("Codex document"),
    )
    .expect("UTF-8 document");
    assert!(codex_document.contains("\"account_id\": \"account-123\""));
    assert!(codex_document.contains("\"type\": \"codex\""));

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
    let claude_document = String::from_utf8(
        serialize_document(&claude, "2026-01-01T00:00:00Z", "").expect("Claude document"),
    )
    .expect("UTF-8 document");
    assert!(!claude_document.contains("account_id"));
    assert!(claude_document.contains("\"type\": \"claude\""));

    let grok = OAuthTokenMaterial::new(
        ProviderKind::Grok,
        "grok-access-secret".into(),
        Some("grok-refresh-secret".into()),
        Some("grok-id-secret".into()),
        Some(1_700_000_000),
        Some("grok-subject".into()),
        Some("grok@example.com".into()),
    )
    .expect("Grok token");
    let grok_document = String::from_utf8(
        serialize_document(&grok, "2026-01-01T00:00:00Z", "2026-01-02T00:00:00Z")
            .expect("Grok document"),
    )
    .expect("UTF-8 document");
    assert!(grok_document.contains("\"sub\": \"grok-subject\""));
    assert!(grok_document.contains("\"type\": \"grok\""));
    assert!(!format!("{grok:?}").contains("grok-access-secret"));
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
