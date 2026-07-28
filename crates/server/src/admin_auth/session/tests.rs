use std::time::{Duration, Instant};

use any2api_domain::{SettingKey, SettingOverrides, SettingValue, SettingsConfiguration};

use super::{
    AdminSessionStore, MAX_ACTIVE_ADMIN_SESSIONS, SessionKey, SessionRecord, random_bytes,
};

#[test]
fn session_record_enforces_idle_and_absolute_deadlines() {
    let settings = session_settings(60, 120);
    let now = Instant::now();
    let key = SessionKey(random_bytes().expect("key"));
    let mut record = SessionRecord::new(random_bytes().expect("csrf"), now);
    assert!(
        record
            .authenticate(key, now + Duration::from_secs(59), settings.admin())
            .is_some()
    );
    assert!(
        record
            .authenticate(key, now + Duration::from_secs(121), settings.admin())
            .is_none()
    );
}

#[test]
fn store_prunes_all_expired_records_on_issue() {
    let settings = session_settings(60, 120);
    let now = Instant::now();
    let mut sessions = AdminSessionStore::default();
    sessions.issue(SessionKey([1; 32]), [11; 32], now, settings.admin());
    sessions.issue(
        SessionKey([2; 32]),
        [12; 32],
        now + Duration::from_secs(121),
        settings.admin(),
    );

    assert_eq!(sessions.len(), 1);
    assert!(
        sessions
            .authenticate(
                SessionKey([1; 32]),
                now + Duration::from_secs(121),
                settings.admin(),
            )
            .is_none()
    );
    assert!(
        sessions
            .authenticate(
                SessionKey([2; 32]),
                now + Duration::from_secs(121),
                settings.admin(),
            )
            .is_some()
    );
}

#[test]
fn store_evicts_the_oldest_record_at_its_hard_cap() {
    let settings = session_settings(10_000, 20_000);
    let now = Instant::now();
    let mut sessions = AdminSessionStore::default();
    for index in 0..MAX_ACTIVE_ADMIN_SESSIONS {
        sessions.issue(
            SessionKey([index as u8; 32]),
            [index as u8 + 64; 32],
            now + Duration::from_secs(index as u64),
            settings.admin(),
        );
    }
    sessions.issue(
        SessionKey([255; 32]),
        [254; 32],
        now + Duration::from_secs(MAX_ACTIVE_ADMIN_SESSIONS as u64),
        settings.admin(),
    );

    assert_eq!(sessions.len(), MAX_ACTIVE_ADMIN_SESSIONS);
    assert!(
        sessions
            .authenticate(
                SessionKey([0; 32]),
                now + Duration::from_secs(MAX_ACTIVE_ADMIN_SESSIONS as u64),
                settings.admin(),
            )
            .is_none()
    );
    assert!(
        sessions
            .authenticate(
                SessionKey([255; 32]),
                now + Duration::from_secs(MAX_ACTIVE_ADMIN_SESSIONS as u64),
                settings.admin(),
            )
            .is_some()
    );
}

fn session_settings(idle_timeout: u64, absolute_timeout: u64) -> SettingsConfiguration {
    SettingsConfiguration::from_overrides(
        SettingOverrides::from_entries([
            (
                SettingKey::AdminSessionIdleTimeout,
                SettingValue::DurationSecs(idle_timeout),
            ),
            (
                SettingKey::AdminSessionAbsoluteTimeout,
                SettingValue::DurationSecs(absolute_timeout),
            ),
        ])
        .expect("session overrides"),
    )
    .expect("session settings")
}
