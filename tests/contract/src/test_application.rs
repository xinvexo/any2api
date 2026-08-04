use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use any2api_runtime::api::{
    ConfigPublisher, PublicRequestService, PublishedSnapshot, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use axum::Router;
use tempfile::TempDir;

use crate::{PublicRequestComponents, TestAdminSession, build_public_request_components};

pub struct TestApplication {
    directory: TempDir,
    storage: Arc<SqliteStore>,
    runtime: Arc<RuntimeRegistry>,
    snapshots: Arc<SnapshotStore>,
    publisher: Arc<ConfigPublisher>,
    components: PublicRequestComponents,
    admin: TestAdminSession,
    web_root: PathBuf,
}

impl TestApplication {
    pub async fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary application directory");
        let storage = Arc::new(
            SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
                .await
                .expect("contract application storage"),
        );
        Self::from_storage(directory, storage).await
    }

    pub async fn from_storage(directory: TempDir, storage: Arc<SqliteStore>) -> Self {
        let components = build_public_request_components().expect("public request components");
        let configuration = storage.load_configuration().await.expect("configuration");
        let runtime = Arc::new(RuntimeRegistry::new());
        let snapshots = Arc::new(SnapshotStore::new(
            PublishedSnapshot::new(
                configuration,
                runtime.as_ref(),
                components.provider_registry(),
            )
            .expect("initial snapshot"),
        ));
        let publisher = Arc::new(
            ConfigPublisher::new(
                Arc::clone(&storage),
                Arc::clone(&snapshots),
                Arc::clone(&runtime),
                components.configuration_capabilities(),
            )
            .expect("configuration publisher"),
        );
        let admin = TestAdminSession::from_snapshot(runtime.as_ref(), snapshots.as_ref()).await;
        let web_root = directory.path().join("web");
        fs::create_dir_all(&web_root).expect("web directory");
        fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
        Self {
            directory,
            storage,
            runtime,
            snapshots,
            publisher,
            components,
            admin,
            web_root,
        }
    }

    pub fn directory(&self) -> &Path {
        self.directory.path()
    }

    pub fn storage(&self) -> Arc<SqliteStore> {
        Arc::clone(&self.storage)
    }

    pub fn runtime(&self) -> Arc<RuntimeRegistry> {
        Arc::clone(&self.runtime)
    }

    pub fn snapshots(&self) -> Arc<SnapshotStore> {
        Arc::clone(&self.snapshots)
    }

    pub fn publisher(&self) -> Arc<ConfigPublisher> {
        Arc::clone(&self.publisher)
    }

    pub const fn components(&self) -> &PublicRequestComponents {
        &self.components
    }

    pub fn state(&self) -> AppState {
        self.state_with_public_requests(self.components.service())
    }

    pub fn state_with_public_requests(
        &self,
        public_requests: Arc<PublicRequestService>,
    ) -> AppState {
        AppState::new(
            self.snapshots(),
            self.runtime(),
            self.publisher(),
            public_requests,
            self.admin.service(),
        )
    }

    pub fn router(&self) -> Router {
        self.router_with_state(self.state())
    }

    fn router_with_state(&self, state: AppState) -> Router {
        self.admin
            .authenticate_loopback_requests(build_router(state, self.web_root.clone()))
    }

    pub fn into_router(self) -> (TempDir, Router, Arc<SqliteStore>) {
        let state = self.state();
        self.into_router_with_state(state)
    }

    pub fn into_router_with_state(self, state: AppState) -> (TempDir, Router, Arc<SqliteStore>) {
        let router = self
            .admin
            .authenticate_loopback_requests(build_router(state, self.web_root));
        (self.directory, router, self.storage)
    }

    pub fn into_raw_router_with_state(
        self,
        state: AppState,
    ) -> (TempDir, Router, Arc<SqliteStore>) {
        let router = build_router(state, self.web_root);
        (self.directory, router, self.storage)
    }
}
