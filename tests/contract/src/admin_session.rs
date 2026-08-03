use std::{net::Ipv4Addr, sync::Arc};

use any2api_domain::AdminSettings;
use any2api_runtime::api::{ProcessLifecycle, RuntimeRegistry, SnapshotStore};
use any2api_server::api::{
    AdminAuthService, AdminCredentialStore, AdminCredentialStoreError, StoredAdminPasswordHash,
};
use async_trait::async_trait;
use axum::{
    Router,
    extract::{ConnectInfo, Request},
    http::{HeaderValue, header::COOKIE},
    middleware::{self, Next},
};
use tokio::sync::{Mutex, OnceCell};

const PASSWORD: &str = "contract administrator password";
static PASSWORD_HASH: OnceCell<String> = OnceCell::const_new();

pub struct TestAdminSession {
    service: Arc<AdminAuthService>,
    cookie: HeaderValue,
    csrf: HeaderValue,
}

impl TestAdminSession {
    pub async fn from_snapshot(runtime: &RuntimeRegistry, snapshots: &SnapshotStore) -> Self {
        let settings = snapshots.load().settings().admin().clone();
        Self::new(runtime.lifecycle(), &settings).await
    }

    pub async fn new(lifecycle: ProcessLifecycle, settings: &AdminSettings) -> Self {
        let store: Arc<dyn AdminCredentialStore> = Arc::new(MemoryAdminCredentialStore::new(Some(
            test_password_hash().await,
        )));
        let service = Arc::new(
            AdminAuthService::load(store, lifecycle)
                .await
                .expect("load contract administrator authentication"),
        );
        let issue = service
            .login(PASSWORD.to_owned(), Ipv4Addr::LOCALHOST.into(), settings)
            .await
            .expect("create contract administrator session");
        let cookie = HeaderValue::from_str(&format!("any2api_admin={}", issue.token()))
            .expect("contract administrator cookie");
        let csrf =
            HeaderValue::from_str(issue.csrf_token()).expect("contract administrator CSRF token");
        Self {
            service,
            cookie,
            csrf,
        }
    }

    pub fn service(&self) -> Arc<AdminAuthService> {
        Arc::clone(&self.service)
    }

    pub fn authenticate_loopback_requests(&self, router: Router) -> Router {
        let cookie = self.cookie.clone();
        let csrf = self.csrf.clone();
        router.layer(middleware::from_fn(
            move |mut request: Request, next: Next| {
                let cookie = cookie.clone();
                let csrf = csrf.clone();
                async move {
                    if is_direct_loopback(&request) && is_admin_path(request.uri().path()) {
                        if !request.headers().contains_key(COOKIE) {
                            request.headers_mut().insert(COOKIE, cookie);
                        }
                        if !request.headers().contains_key("x-csrf-token") {
                            request.headers_mut().insert("x-csrf-token", csrf);
                        }
                    }
                    next.run(request).await
                }
            },
        ))
    }
}

fn is_direct_loopback(request: &Request) -> bool {
    request
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .is_some_and(|ConnectInfo(peer)| peer.ip().to_canonical().is_loopback())
}

fn is_admin_path(path: &str) -> bool {
    path == "/api/admin" || path.starts_with("/api/admin/")
}

async fn test_password_hash() -> String {
    PASSWORD_HASH
        .get_or_init(|| async {
            let store = Arc::new(MemoryAdminCredentialStore::new(None));
            let service = AdminAuthService::load(store.clone(), ProcessLifecycle::new())
                .await
                .expect("load password-hash fixture service");
            assert!(
                service
                    .initialize_if_missing(PASSWORD.to_owned())
                    .await
                    .expect("initialize password-hash fixture")
            );
            store
                .load()
                .await
                .expect("load password-hash fixture")
                .expect("initialized password-hash fixture")
                .as_str()
                .to_owned()
        })
        .await
        .clone()
}

struct MemoryAdminCredentialStore {
    password_hash: Mutex<Option<StoredAdminPasswordHash>>,
}

impl MemoryAdminCredentialStore {
    fn new(password_hash: Option<String>) -> Self {
        Self {
            password_hash: Mutex::new(password_hash.map(StoredAdminPasswordHash::new)),
        }
    }
}

#[async_trait]
impl AdminCredentialStore for MemoryAdminCredentialStore {
    async fn load(&self) -> Result<Option<StoredAdminPasswordHash>, AdminCredentialStoreError> {
        Ok(self.password_hash.lock().await.clone())
    }

    async fn initialize(&self, password_hash: &str) -> Result<bool, AdminCredentialStoreError> {
        let mut stored = self.password_hash.lock().await;
        if stored.is_some() {
            return Ok(false);
        }
        *stored = Some(StoredAdminPasswordHash::new(password_hash.to_owned()));
        Ok(true)
    }

    async fn replace(
        &self,
        expected_password_hash: &str,
        new_password_hash: &str,
    ) -> Result<bool, AdminCredentialStoreError> {
        let mut stored = self.password_hash.lock().await;
        if stored
            .as_ref()
            .is_none_or(|value| value.as_str() != expected_password_hash)
        {
            return Ok(false);
        }
        *stored = Some(StoredAdminPasswordHash::new(new_password_hash.to_owned()));
        Ok(true)
    }
}
