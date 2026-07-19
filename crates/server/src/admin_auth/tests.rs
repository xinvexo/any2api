use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use any2api_domain::{SettingKey, SettingOverrides, SettingValue, SettingsConfiguration};
use async_trait::async_trait;
use axum::http::{HeaderMap, HeaderValue};
use ipnet::IpNet;

use super::{
    AdminAuthService, AdminCredentialStore, AdminCredentialStoreError, AdminNetworkPolicy,
    StoredAdminPasswordHash,
    session::{SessionKey, SessionRecord, random_bytes},
};

#[tokio::test]
async fn password_login_session_csrf_and_logout_are_server_side() {
    let store = Arc::new(MemoryStore::default());
    let service = AdminAuthService::load(store).await.expect("auth service");
    let setup_token = service.setup_token().await.expect("setup token");
    assert!(matches!(
        service
            .initialize_with_setup_token(
                "correct horse battery staple".to_owned(),
                "invalid-token",
            )
            .await,
        Err(super::AdminAuthError::InvalidSetupToken)
    ));
    assert!(
        service
            .initialize_with_setup_token("correct horse battery staple".to_owned(), &setup_token,)
            .await
            .expect("initialize")
    );
    assert!(
        !service
            .initialize_if_missing("short".to_owned())
            .await
            .expect("existing credential ignores environment value")
    );
    let settings = SettingsConfiguration::defaults();
    let issue = service
        .login(
            "correct horse battery staple".to_owned(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            settings.admin(),
        )
        .await
        .expect("login");
    let session = service
        .authenticate(issue.token(), settings.admin())
        .await
        .expect("session");
    assert!(session.csrf_matches(issue.csrf_token()));
    assert!(!session.csrf_matches("wrong"));
    service.logout(session).await;
    assert!(
        service
            .authenticate(issue.token(), settings.admin())
            .await
            .is_none()
    );
}

#[tokio::test]
async fn cancelled_login_keeps_its_argon2_permit_until_blocking_work_finishes() {
    let service = Arc::new(
        AdminAuthService::load(Arc::new(MemoryStore::default()))
            .await
            .expect("auth service"),
    );
    service
        .initialize_if_missing("correct horse battery staple".to_owned())
        .await
        .expect("initialize");
    let settings = SettingsConfiguration::defaults().admin().clone();
    let login_service = Arc::clone(&service);
    let login = tokio::spawn(async move {
        login_service
            .login(
                "incorrect password".to_owned(),
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                &settings,
            )
            .await
    });
    wait_for_available_password_checks(&service, 3).await;
    login.abort();
    assert_eq!(service.available_password_checks(), 3);
    wait_for_available_password_checks(&service, 4).await;
    assert_eq!(
        service.failure_count(IpAddr::V4(Ipv4Addr::LOCALHOST)).await,
        1
    );
}

#[tokio::test]
async fn cancelled_setup_keeps_the_single_hash_permit_and_does_not_initialize() {
    let service = Arc::new(
        AdminAuthService::load(Arc::new(MemoryStore::default()))
            .await
            .expect("auth service"),
    );
    let setup_token = service.setup_token().await.expect("setup token");
    let setup_service = Arc::clone(&service);
    let setup = tokio::spawn(async move {
        setup_service
            .initialize_with_setup_token("correct horse battery staple".to_owned(), &setup_token)
            .await
    });
    wait_for_available_setup_checks(&service, 0).await;
    setup.abort();
    assert_eq!(service.available_setup_checks(), 0);
    wait_for_available_setup_checks(&service, 1).await;
    assert!(!service.is_initialized().await);
}

#[test]
fn session_record_enforces_idle_and_absolute_deadlines() {
    let settings = SettingsConfiguration::from_overrides(
        SettingOverrides::from_entries([
            (
                SettingKey::AdminSessionIdleTimeout,
                SettingValue::DurationMs(60_000),
            ),
            (
                SettingKey::AdminSessionAbsoluteTimeout,
                SettingValue::DurationMs(120_000),
            ),
        ])
        .expect("overrides"),
    )
    .expect("settings");
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
fn forwarded_headers_only_apply_to_explicit_trusted_proxy_cidrs() {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.8"));
    headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
    let peer = SocketAddr::from(([127, 0, 0, 1], 41000));

    let direct = AdminNetworkPolicy::default()
        .resolve(Some(peer), &headers)
        .expect("direct source");
    assert!(direct.is_loopback());
    assert!(!direct.is_secure());

    let trusted = AdminNetworkPolicy::new(vec!["127.0.0.0/8".parse::<IpNet>().expect("cidr")])
        .resolve(Some(peer), &headers)
        .expect("trusted source");
    assert_eq!(
        trusted.client_ip(),
        "203.0.113.8".parse::<IpAddr>().expect("ip")
    );
    assert!(trusted.is_secure());
    assert!(trusted.through_trusted_proxy());

    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("127.0.0.1, 203.0.113.8"),
    );
    let spoofed = AdminNetworkPolicy::new(vec!["127.0.0.0/8".parse::<IpNet>().expect("cidr")])
        .resolve(Some(peer), &headers)
        .expect("trusted chain");
    assert_eq!(
        spoofed.client_ip(),
        "203.0.113.8".parse::<IpAddr>().expect("ip")
    );
}

#[derive(Default)]
struct MemoryStore {
    value: Mutex<Option<String>>,
}

#[async_trait]
impl AdminCredentialStore for MemoryStore {
    async fn load(&self) -> Result<Option<StoredAdminPasswordHash>, AdminCredentialStoreError> {
        Ok(self
            .value
            .lock()
            .expect("memory store")
            .clone()
            .map(StoredAdminPasswordHash::new))
    }

    async fn initialize(&self, password_hash: &str) -> Result<bool, AdminCredentialStoreError> {
        let mut value = self.value.lock().expect("memory store");
        if value.is_some() {
            return Ok(false);
        }
        *value = Some(password_hash.to_owned());
        Ok(true)
    }
}

async fn wait_for_available_password_checks(service: &AdminAuthService, expected: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while service.available_password_checks() != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("password check permit state");
}

async fn wait_for_available_setup_checks(service: &AdminAuthService, expected: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while service.available_setup_checks() != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("setup check permit state");
}
