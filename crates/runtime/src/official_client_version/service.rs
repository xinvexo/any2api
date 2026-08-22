use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use any2api_domain::ProviderKind;
use any2api_provider::api::{OfficialClientVersion, ProviderError, ProviderRegistry};
use any2api_storage::api::{
    OfficialClientVersionRepository, SqliteStore, StorageError, StoredOfficialClientVersion,
};
use any2api_transport::api::{
    EndpointNetworkPolicy, TransportError, TransportIsolationKey, TransportManager,
    TransportRequest, TransportTrafficClass,
};
use bytes::{Bytes, BytesMut};
use futures_util::{StreamExt, future::join_all};
use thiserror::Error;
use tokio::{sync::Mutex as AsyncMutex, time::timeout};

use crate::{configuration::SnapshotStore, lifecycle::ProcessLifecycle};

const REFRESH_CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);
const SOURCE_REFRESH_INTERVAL_SECS: i64 = 6 * 60 * 60;
const VERSION_READ_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_VERSION_RESPONSE_BYTES: usize = 128 * 1024;

pub struct OfficialClientVersionService {
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
    snapshots: Arc<SnapshotStore>,
    storage: Arc<SqliteStore>,
    fetched_at_by_provider: Mutex<HashMap<ProviderKind, i64>>,
    synchronize_gate: AsyncMutex<()>,
    worker_started: AtomicBool,
}

impl OfficialClientVersionService {
    #[must_use]
    pub fn new(
        providers: Arc<ProviderRegistry>,
        transport: Arc<dyn TransportManager>,
        snapshots: Arc<SnapshotStore>,
        storage: Arc<SqliteStore>,
    ) -> Arc<Self> {
        Arc::new(Self {
            providers,
            transport,
            snapshots,
            storage,
            fetched_at_by_provider: Mutex::new(HashMap::new()),
            synchronize_gate: AsyncMutex::new(()),
            worker_started: AtomicBool::new(false),
        })
    }

    pub async fn initialize(&self) -> Result<(), OfficialClientVersionError> {
        let _guard = self.synchronize_gate.lock().await;
        self.load_from_storage().await?;
        self.refresh_stale_sources().await;
        Ok(())
    }

    pub fn start(self: &Arc<Self>, lifecycle: &ProcessLifecycle) -> bool {
        if self
            .worker_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        let service = Arc::clone(self);
        drop(lifecycle.spawn_until_draining(async move {
            loop {
                tokio::time::sleep(REFRESH_CHECK_INTERVAL).await;
                service.synchronize().await;
            }
        }));
        true
    }

    async fn synchronize(&self) {
        let _guard = self.synchronize_gate.lock().await;
        self.refresh_stale_sources().await;
    }

    async fn load_from_storage(&self) -> Result<(), OfficialClientVersionError> {
        let rows = self.storage.load_official_client_versions().await?;
        let mut fetched_at_by_provider = HashMap::with_capacity(rows.len());
        for row in rows {
            if let Some(versioned) = self
                .providers
                .get(row.provider_kind)
                .and_then(|driver| driver.official_client_version())
            {
                let version = OfficialClientVersion::new(row.version.clone()).map_err(|_| {
                    OfficialClientVersionError::InvalidStoredVersion(row.provider_kind)
                })?;
                versioned.publish_official_client_version(version);
            }
            fetched_at_by_provider.insert(row.provider_kind, row.fetched_at);
        }
        *self
            .fetched_at_by_provider
            .lock()
            .expect("official client version state lock poisoned") = fetched_at_by_provider;
        Ok(())
    }

    async fn refresh_stale_sources(&self) {
        let now = unix_timestamp();
        let fetched_at_by_provider = self
            .fetched_at_by_provider
            .lock()
            .expect("official client version state lock poisoned")
            .clone();
        let due = self
            .providers
            .iter()
            .filter_map(|(kind, driver)| {
                driver.official_client_version()?;
                let stale = fetched_at_by_provider.get(kind).is_none_or(|fetched_at| {
                    now.saturating_sub(*fetched_at) >= SOURCE_REFRESH_INTERVAL_SECS
                });
                stale.then(|| (*kind, Arc::clone(driver)))
            })
            .collect::<Vec<_>>();

        let results = join_all(due.into_iter().map(|(provider, driver)| async move {
            (provider, self.fetch(provider, driver).await)
        }))
        .await;

        for (provider, result) in results {
            match result {
                Ok(version) => {
                    if let Err(error) = self
                        .persist_then_publish(provider, version, unix_timestamp())
                        .await
                    {
                        tracing::warn!(provider = provider.as_str(), error = %error, "official client version persistence failed");
                    }
                }
                Err(error) => {
                    tracing::warn!(provider = provider.as_str(), error = %error, "official client version fetch failed");
                }
            }
        }
    }

    async fn fetch(
        &self,
        provider: ProviderKind,
        driver: Arc<dyn any2api_provider::api::ProviderDriver>,
    ) -> Result<OfficialClientVersion, OfficialClientVersionError> {
        let versioned = driver
            .official_client_version()
            .ok_or(OfficialClientVersionError::Unavailable(vec![provider]))?;
        let plan = versioned
            .latest_official_client_version_plan()
            .map_err(|source| OfficialClientVersionError::Provider { provider, source })?;
        let snapshot = self.snapshots.load();
        let proxy = snapshot
            .transport_proxy(snapshot.proxies().global_proxy_id())
            .ok_or(OfficialClientVersionError::ProxyUnavailable)?;
        let request = TransportRequest {
            method: plan.method,
            uri: plan
                .url
                .as_str()
                .parse()
                .map_err(|_| OfficialClientVersionError::InvalidEndpoint(provider))?,
            headers: plan.headers,
            body: Bytes::from(plan.body),
            isolation: TransportIsolationKey::ephemeral(TransportTrafficClass::Diagnostic),
            network_policy: EndpointNetworkPolicy::new()
                .with_strict_ssrf(snapshot.settings().upstream().strict_ssrf()),
            read_timeout: VERSION_READ_TIMEOUT,
        };
        let response = self
            .transport
            .execute(proxy, request)
            .await
            .map_err(|source| OfficialClientVersionError::Transport { provider, source })?;
        if !response.status.is_success() {
            return Err(OfficialClientVersionError::Rejected {
                provider,
                status: response.status.as_u16(),
            });
        }
        let body = collect(response.body).await.map_err(|error| match error {
            VersionBodyReadError::Timeout => OfficialClientVersionError::ReadTimeout(provider),
            VersionBodyReadError::TooLarge => {
                OfficialClientVersionError::ResponseTooLarge(provider)
            }
            VersionBodyReadError::Transport(source) => {
                OfficialClientVersionError::Transport { provider, source }
            }
        })?;
        versioned
            .parse_latest_official_client_version(&body)
            .map_err(|source| OfficialClientVersionError::Provider { provider, source })
    }

    async fn persist_then_publish(
        &self,
        provider: ProviderKind,
        version: OfficialClientVersion,
        fetched_at: i64,
    ) -> Result<(), OfficialClientVersionError> {
        let stored = StoredOfficialClientVersion {
            provider_kind: provider,
            version: version.as_str().to_owned(),
            fetched_at,
        };
        self.storage.upsert_official_client_version(&stored).await?;
        let versioned = self
            .providers
            .get(provider)
            .and_then(|driver| driver.official_client_version())
            .ok_or(OfficialClientVersionError::Unavailable(vec![provider]))?;
        versioned.publish_official_client_version(version);
        self.fetched_at_by_provider
            .lock()
            .expect("official client version state lock poisoned")
            .insert(provider, fetched_at);
        Ok(())
    }
}

async fn collect(
    mut body: any2api_transport::api::BoxByteStream,
) -> Result<Bytes, VersionBodyReadError> {
    let mut collected = BytesMut::new();
    loop {
        let next = timeout(VERSION_READ_TIMEOUT, body.next())
            .await
            .map_err(|_| VersionBodyReadError::Timeout)?;
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk?;
        if collected.len().saturating_add(chunk.len()) > MAX_VERSION_RESPONSE_BYTES {
            return Err(VersionBodyReadError::TooLarge);
        }
        collected.extend_from_slice(&chunk);
    }
    Ok(collected.freeze())
}

#[derive(Debug, Error)]
enum VersionBodyReadError {
    #[error("response body read timed out")]
    Timeout,
    #[error("response body exceeded the size limit")]
    TooLarge,
    #[error(transparent)]
    Transport(#[from] TransportError),
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(0)
}

#[derive(Debug, Error)]
pub enum OfficialClientVersionError {
    #[error("official client version storage failed: {0}")]
    Storage(#[from] StorageError),
    #[error("stored official client version is invalid for provider {0:?}")]
    InvalidStoredVersion(ProviderKind),
    #[error("official client version is unavailable for providers {0:?}")]
    Unavailable(Vec<ProviderKind>),
    #[error("global proxy is unavailable")]
    ProxyUnavailable,
    #[error("official client version endpoint is invalid for provider {0:?}")]
    InvalidEndpoint(ProviderKind),
    #[error("official client version request failed for provider {provider:?}: {source}")]
    Transport {
        provider: ProviderKind,
        #[source]
        source: TransportError,
    },
    #[error("official client version response read timed out for provider {0:?}")]
    ReadTimeout(ProviderKind),
    #[error("official client version response exceeded the size limit for provider {0:?}")]
    ResponseTooLarge(ProviderKind),
    #[error("official client version endpoint rejected provider {provider:?} with HTTP {status}")]
    Rejected { provider: ProviderKind, status: u16 },
    #[error("official client version response is invalid for provider {provider:?}: {source}")]
    Provider {
        provider: ProviderKind,
        #[source]
        source: ProviderError,
    },
}

#[cfg(test)]
mod tests;
