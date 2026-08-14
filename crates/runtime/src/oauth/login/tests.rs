use std::time::Instant;

use any2api_domain::{OAuthProxySelection, ProviderKind};
use any2api_provider::api::OAuthTokenMaterial;

use super::{
    callback,
    session::{
        AuthorizationCodeSession, DeviceCodeSession, DevicePollAcquisition, MAX_ACTIVE_SESSIONS,
        OAuthSessionRegistry,
    },
};

#[test]
fn oauth_session_is_consumed_once() {
    let prepared = AuthorizationCodeSession::prepare(
        ProviderKind::Codex,
        OAuthProxySelection::Global,
        "http://localhost:1455/auth/callback",
        Instant::now(),
    )
    .expect("session should be prepared");
    let id = prepared.id.clone();
    let store = OAuthSessionRegistry::default();
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
    let prepared = DeviceCodeSession::prepare(
        ProviderKind::Grok,
        OAuthProxySelection::Global,
        "device-secret",
        1_800,
        5,
        now,
    )
    .expect("device session should be prepared");
    let id = prepared.id.clone();
    let store = OAuthSessionRegistry::default();
    store
        .insert_device_code(id.clone(), prepared.session, now)
        .expect("device session should be inserted");

    assert!(store.take_authorization_code(&id, now).is_err());
    let DevicePollAcquisition::Ready(lease) = store
        .acquire_device_poll(&id, now)
        .expect("device poll should acquire a lease")
    else {
        panic!("first poll must be ready");
    };
    assert_eq!(lease.device_code(), "device-secret");
    assert_eq!(lease.proxy_selection(), OAuthProxySelection::Global);
    assert_eq!(lease.restore(false).expect("pending poll"), 5);

    assert!(matches!(
        store
            .acquire_device_poll(&id, Instant::now())
            .expect("pending session should remain available"),
        DevicePollAcquisition::Pending {
            retry_after_seconds: 5
        }
    ));
}

#[test]
fn dropped_device_poll_lease_restores_and_serializes_the_session() {
    let now = Instant::now();
    let prepared = DeviceCodeSession::prepare(
        ProviderKind::Grok,
        OAuthProxySelection::Global,
        "device-secret",
        1_800,
        5,
        now,
    )
    .expect("device session");
    let id = prepared.id.clone();
    let store = OAuthSessionRegistry::default();
    store
        .insert_device_code(id.clone(), prepared.session, now)
        .expect("insert session");

    let DevicePollAcquisition::Ready(lease) =
        store.acquire_device_poll(&id, now).expect("first lease")
    else {
        panic!("first poll must be ready");
    };
    assert!(matches!(
        store
            .acquire_device_poll(&id, now)
            .expect("serialized poll"),
        DevicePollAcquisition::Pending {
            retry_after_seconds: 1
        }
    ));

    drop(lease);
    assert!(matches!(
        store
            .acquire_device_poll(&id, Instant::now())
            .expect("restored session"),
        DevicePollAcquisition::Pending {
            retry_after_seconds: 5
        }
    ));
}

#[test]
fn device_poll_leases_remain_inside_the_global_session_capacity() {
    let now = Instant::now();
    let store = OAuthSessionRegistry::default();
    let mut ids = Vec::new();
    for index in 0..MAX_ACTIVE_SESSIONS {
        let prepared = DeviceCodeSession::prepare(
            ProviderKind::Grok,
            OAuthProxySelection::Global,
            &format!("device-secret-{index}"),
            1_800,
            5,
            now,
        )
        .expect("device session");
        ids.push(prepared.id.clone());
        store
            .insert_device_code(prepared.id, prepared.session, now)
            .expect("capacity slot");
    }

    let leases = ids
        .iter()
        .map(
            |id| match store.acquire_device_poll(id, now).expect("poll lease") {
                DevicePollAcquisition::Ready(lease) => lease,
                DevicePollAcquisition::Pending { .. } => panic!("poll must be ready"),
            },
        )
        .collect::<Vec<_>>();
    assert_eq!(store.active_len(), MAX_ACTIVE_SESSIONS);

    let overflow = DeviceCodeSession::prepare(
        ProviderKind::Grok,
        OAuthProxySelection::Global,
        "overflow-device-secret",
        1_800,
        5,
        now,
    )
    .expect("overflow session");
    assert!(matches!(
        store.insert_device_code(overflow.id, overflow.session, now),
        Err(crate::oauth::error::OAuthError::SessionCapacity)
    ));
    drop(leases);
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
        crate::oauth::error::OAuthError::StateMismatch
    ));

    let redirect_error = callback::parse(
        "http://localhost:1455/other?code=abc&state=expected",
        redirect,
        "expected",
    )
    .expect_err("redirect target mismatch must be rejected");
    assert!(matches!(
        redirect_error,
        crate::oauth::error::OAuthError::InvalidCallback
    ));
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
