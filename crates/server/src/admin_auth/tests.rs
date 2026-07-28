use std::{
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
    time::Duration,
};

use any2api_domain::SettingsConfiguration;
use any2api_runtime::api::ProcessLifecycle;
use async_trait::async_trait;
use tokio::sync::Barrier;

use super::{
    AdminAuthService, AdminCredentialStore, AdminCredentialStoreError, StoredAdminPasswordHash,
};

#[tokio::test]
async fn password_login_session_csrf_and_logout_are_server_side() {
    let store = Arc::new(MemoryStore::default());
    let service = test_service(store).await;
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
async fn password_rotation_reissues_one_session_and_revokes_the_rest() {
    let store = Arc::new(MemoryStore::default());
    let service = test_service(store).await;
    service
        .initialize_if_missing("correct horse battery staple".to_owned())
        .await
        .expect("initialize");
    let settings = SettingsConfiguration::defaults();
    let first = service
        .login(
            "correct horse battery staple".to_owned(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            settings.admin(),
        )
        .await
        .expect("first login");
    let second = service
        .login(
            "correct horse battery staple".to_owned(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            settings.admin(),
        )
        .await
        .expect("second login");

    let replacement = service
        .rotate_password(
            "correct horse battery staple".to_owned(),
            "new correct horse battery staple".to_owned(),
        )
        .await
        .expect("rotate");
    assert!(
        service
            .authenticate(replacement.token(), settings.admin())
            .await
            .is_some()
    );
    assert!(
        service
            .authenticate(first.token(), settings.admin())
            .await
            .is_none()
    );
    assert!(
        service
            .authenticate(second.token(), settings.admin())
            .await
            .is_none()
    );
    assert!(matches!(
        service
            .login(
                "correct horse battery staple".to_owned(),
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                settings.admin(),
            )
            .await,
        Err(super::AdminAuthError::InvalidCredentials)
    ));
    assert!(
        service
            .login(
                "new correct horse battery staple".to_owned(),
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                settings.admin(),
            )
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn wrong_rotation_password_preserves_the_existing_session() {
    let store = Arc::new(MemoryStore::default());
    let service = test_service(store).await;
    service
        .initialize_if_missing("correct horse battery staple".to_owned())
        .await
        .expect("initialize");
    let settings = SettingsConfiguration::defaults();
    let existing = service
        .login(
            "correct horse battery staple".to_owned(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            settings.admin(),
        )
        .await
        .expect("login");
    assert!(matches!(
        service
            .rotate_password("wrong password".to_owned(), "new password value".to_owned())
            .await,
        Err(super::AdminAuthError::CurrentPasswordInvalid)
    ));
    assert!(
        service
            .authenticate(existing.token(), settings.admin())
            .await
            .is_some()
    );
    assert!(
        service
            .login(
                "correct horse battery staple".to_owned(),
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                settings.admin(),
            )
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn old_password_login_cannot_survive_a_concurrent_rotation() {
    let service = Arc::new(test_service(Arc::new(MemoryStore::default())).await);
    service
        .initialize_if_missing("correct horse battery staple".to_owned())
        .await
        .expect("initialize");
    let settings = SettingsConfiguration::defaults().admin().clone();
    let barrier = Arc::new(Barrier::new(2));

    let login_service = Arc::clone(&service);
    let login_barrier = Arc::clone(&barrier);
    let login = tokio::spawn(async move {
        login_barrier.wait().await;
        login_service
            .login(
                "correct horse battery staple".to_owned(),
                IpAddr::V4(Ipv4Addr::LOCALHOST),
                &settings,
            )
            .await
    });
    let rotation_service = Arc::clone(&service);
    let rotation = tokio::spawn(async move {
        barrier.wait().await;
        rotation_service
            .rotate_password(
                "correct horse battery staple".to_owned(),
                "new correct horse battery staple".to_owned(),
            )
            .await
    });

    let old_login = login.await.expect("login task");
    let replacement = rotation
        .await
        .expect("rotation task")
        .expect("password rotation");
    let current_settings = SettingsConfiguration::defaults();
    if let Ok(issue) = old_login {
        assert!(
            service
                .authenticate(issue.token(), current_settings.admin())
                .await
                .is_none()
        );
    }
    assert!(
        service
            .authenticate(replacement.token(), current_settings.admin())
            .await
            .is_some()
    );
}

#[tokio::test]
async fn cancelled_login_keeps_its_argon2_permit_until_blocking_work_finishes() {
    let service = Arc::new(test_service(Arc::new(MemoryStore::default())).await);
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
    let service = Arc::new(test_service(Arc::new(MemoryStore::default())).await);
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

    async fn replace(
        &self,
        expected_password_hash: &str,
        new_password_hash: &str,
    ) -> Result<bool, AdminCredentialStoreError> {
        let mut value = self.value.lock().expect("memory store");
        if value.as_deref() != Some(expected_password_hash) {
            return Ok(false);
        }
        *value = Some(new_password_hash.to_owned());
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

async fn test_service(store: Arc<dyn AdminCredentialStore>) -> AdminAuthService {
    AdminAuthService::load(store, ProcessLifecycle::new())
        .await
        .expect("auth service")
}
