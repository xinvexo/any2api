use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex, Weak},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{OAuthAccountId, OAuthProxySelection, ProviderKind, RoutingCredentialId};
use any2api_provider::api::{OAuthModelCatalogScope, OAuthTokenMaterial};
use any2api_storage::api::{OAuthModelCatalogSnapshotRepository, StoredOAuthModelCatalogSnapshot};
use any2api_transport::api::{TransportIsolationKey, TransportTrafficClass};
use tokio::sync::Mutex as AsyncMutex;

use crate::{
    configuration::PublishedSnapshot,
    oauth::{error::OAuthError, login::token_request},
};

use super::{coordinator::OAuthQuotaService, rejection::RequestContext, types::OAuthQuotaError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthModelCatalogSnapshot {
    models: Vec<String>,
    fetched_at: i64,
}

impl OAuthModelCatalogSnapshot {
    #[must_use]
    pub fn models(&self) -> &[String] {
        &self.models
    }

    #[must_use]
    pub const fn fetched_at(&self) -> i64 {
        self.fetched_at
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OAuthModelCatalogRefreshSummary {
    refreshed_scopes: usize,
    failed_scopes: usize,
}

impl OAuthModelCatalogRefreshSummary {
    #[must_use]
    pub const fn refreshed_scopes(self) -> usize {
        self.refreshed_scopes
    }

    #[must_use]
    pub const fn failed_scopes(self) -> usize {
        self.failed_scopes
    }
}

pub(super) struct OAuthModelCatalogPersistence {
    repository: Arc<dyn OAuthModelCatalogSnapshotRepository>,
}

impl OAuthModelCatalogPersistence {
    pub(super) fn new(repository: Arc<dyn OAuthModelCatalogSnapshotRepository>) -> Self {
        Self { repository }
    }

    async fn load_all(&self) -> Result<Vec<StoredOAuthModelCatalogSnapshot>, OAuthQuotaError> {
        self.repository
            .load_oauth_model_catalog_snapshots()
            .await
            .map_err(|error| OAuthQuotaError::ModelCatalogPersistence(Arc::new(error)))
    }

    async fn store(
        &self,
        snapshot: &StoredOAuthModelCatalogSnapshot,
    ) -> Result<(), OAuthQuotaError> {
        self.repository
            .upsert_oauth_model_catalog_snapshot(snapshot)
            .await
            .map_err(|error| OAuthQuotaError::ModelCatalogPersistence(Arc::new(error)))
    }
}

#[derive(Default)]
pub(super) struct OAuthModelCatalogGates {
    gates: Mutex<HashMap<OAuthModelCatalogScope, Weak<AsyncMutex<()>>>>,
}

impl OAuthModelCatalogGates {
    fn get(&self, scope: OAuthModelCatalogScope) -> Arc<AsyncMutex<()>> {
        let mut gates = self
            .gates
            .lock()
            .expect("OAuth model catalog gate lock poisoned");
        gates.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = gates.get(&scope).and_then(Weak::upgrade) {
            return gate;
        }
        let gate = Arc::new(AsyncMutex::new(()));
        gates.insert(scope, Arc::downgrade(&gate));
        gate
    }
}

impl OAuthQuotaService {
    pub(in crate::oauth) async fn fetch_model_catalog_for_login(
        &self,
        provider: ProviderKind,
        proxy_selection: OAuthProxySelection,
        token: &OAuthTokenMaterial,
    ) -> Result<Vec<String>, OAuthError> {
        let driver = self
            .providers
            .get(provider)
            .ok_or(OAuthError::ProviderUnavailable)?;
        let routing = driver
            .oauth_routing()
            .ok_or(OAuthError::UnsupportedProvider(provider))?;
        let scope = routing.oauth_model_catalog_scope(token)?;
        let plan = routing.oauth_model_catalog_plan(token)?;
        let snapshot = self.publisher.current_snapshot();
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_selection(proxy_selection)
            .ok_or(OAuthError::PublishedProxyUnavailable)?;
        let response = token_request::execute_model_catalog_response(
            self.transport.as_ref(),
            self.control_plane.as_ref(),
            provider,
            proxy,
            snapshot.settings().upstream().strict_ssrf(),
            TransportIsolationKey::ephemeral(TransportTrafficClass::OAuthToken),
            plan,
        )
        .await?;
        if !response.status.is_success() {
            return Err(OAuthError::ModelCatalogRejected(response.status.as_u16()));
        }
        let models = routing
            .parse_oauth_model_catalog(&response.body)
            .map_err(OAuthError::from_model_catalog_response_error)?;
        self.store_catalog(scope, models.clone())
            .await
            .map_err(|_| OAuthError::ModelCatalogPersistence)?;
        Ok(models)
    }

    pub(in crate::oauth) async fn refresh_model_catalog(
        &self,
        id: OAuthAccountId,
    ) -> Result<(), OAuthQuotaError> {
        let snapshot = self.publisher.current_snapshot();
        let scope = self.scope_for_account(snapshot.as_ref(), id)?;
        self.refresh_scope(scope, id).await
    }

    pub(in crate::oauth) async fn refresh_model_catalogs(
        &self,
        ids: &[OAuthAccountId],
    ) -> OAuthModelCatalogRefreshSummary {
        let snapshot = self.publisher.current_snapshot();
        let mut representatives = BTreeMap::<OAuthModelCatalogScope, OAuthAccountId>::new();
        for id in ids {
            let Ok(scope) = self.scope_for_account(snapshot.as_ref(), *id) else {
                continue;
            };
            representatives
                .entry(scope)
                .and_modify(|current| *current = (*current).min(*id))
                .or_insert(*id);
        }
        let mut summary = OAuthModelCatalogRefreshSummary::default();
        for (scope, id) in representatives {
            match self.refresh_scope(scope, id).await {
                Ok(()) => summary.refreshed_scopes += 1,
                Err(error) => {
                    summary.failed_scopes += 1;
                    tracing::warn!(oauth_account_id = %id, error = %error, "manual OAuth model catalog refresh failed");
                }
            }
        }
        summary
    }

    pub(in crate::oauth) async fn model_catalogs_for_accounts(
        &self,
        snapshot: &PublishedSnapshot,
    ) -> Result<HashMap<OAuthAccountId, OAuthModelCatalogSnapshot>, OAuthQuotaError> {
        let stored = self.model_catalog_persistence.load_all().await?;
        let by_scope = stored
            .into_iter()
            .map(|snapshot| {
                (
                    (snapshot.provider_kind, snapshot.directory_scope),
                    OAuthModelCatalogSnapshot {
                        models: snapshot.models,
                        fetched_at: snapshot.fetched_at,
                    },
                )
            })
            .collect::<HashMap<_, _>>();
        let mut catalogs = HashMap::new();
        for account in snapshot.oauth_accounts().accounts() {
            let Ok(scope) = self.scope_for_account(snapshot, account.id()) else {
                continue;
            };
            if let Some(catalog) = by_scope.get(&(scope.provider(), scope.directory_scope().into()))
            {
                catalogs.insert(account.id(), catalog.clone());
            }
        }
        Ok(catalogs)
    }

    async fn refresh_scope(
        &self,
        scope: OAuthModelCatalogScope,
        representative: OAuthAccountId,
    ) -> Result<(), OAuthQuotaError> {
        let gate = self.model_catalog_gates.get(scope);
        let _guard = gate.lock().await;
        self.fetch_catalog_for_account(representative).await
    }

    async fn fetch_catalog_for_account(&self, id: OAuthAccountId) -> Result<(), OAuthQuotaError> {
        let snapshot = self.publisher.current_snapshot();
        let account = snapshot
            .oauth_accounts()
            .get(id)
            .ok_or(OAuthQuotaError::AccountNotFound)?;
        let driver = self
            .providers
            .get(account.provider_kind())
            .ok_or(OAuthQuotaError::ProviderUnavailable)?;
        let routing = driver
            .oauth_routing()
            .ok_or(OAuthQuotaError::UnsupportedProvider)?;
        let quota = driver
            .oauth_quota()
            .ok_or(OAuthQuotaError::UnsupportedProvider)?;
        let binding = snapshot
            .credential_runtime(RoutingCredentialId::oauth_account(id))
            .ok_or(OAuthQuotaError::RuntimeUnavailable)?;
        let token = binding
            .generation()
            .oauth_token()
            .ok_or(OAuthQuotaError::TokenMaterialUnavailable)?;
        let scope = routing
            .oauth_model_catalog_scope(token.as_ref())
            .map_err(OAuthQuotaError::Provider)?;
        let plan = routing
            .oauth_model_catalog_plan(token.as_ref())
            .map_err(OAuthQuotaError::Provider)?;
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account(id)
            .ok_or(OAuthQuotaError::ProxyUnavailable)?;
        let request = RequestContext::new(
            quota,
            self.transport.as_ref(),
            self.control_plane.as_ref(),
            proxy,
            snapshot.settings().upstream().strict_ssrf(),
            Duration::from_secs(snapshot.settings().upstream().read_timeout_secs()),
            binding.transport_isolation(TransportTrafficClass::OAuthQuota),
        );
        let response = request.execute_model_catalog(plan).await?;
        if !response.status.is_success() {
            return Err(request.rejection(&response));
        }
        let models = routing
            .parse_oauth_model_catalog(&response.body)
            .map_err(OAuthQuotaError::Provider)?;
        self.store_catalog(scope, models).await
    }

    fn scope_for_account(
        &self,
        snapshot: &PublishedSnapshot,
        id: OAuthAccountId,
    ) -> Result<OAuthModelCatalogScope, OAuthQuotaError> {
        let account = snapshot
            .oauth_accounts()
            .get(id)
            .ok_or(OAuthQuotaError::AccountNotFound)?;
        let driver = self
            .providers
            .get(account.provider_kind())
            .ok_or(OAuthQuotaError::ProviderUnavailable)?;
        let routing = driver
            .oauth_routing()
            .ok_or(OAuthQuotaError::UnsupportedProvider)?;
        let token = snapshot
            .oauth_token_material(id)
            .ok_or(OAuthQuotaError::TokenMaterialUnavailable)?;
        routing
            .oauth_model_catalog_scope(token.as_ref())
            .map_err(OAuthQuotaError::Provider)
    }

    async fn store_catalog(
        &self,
        scope: OAuthModelCatalogScope,
        models: Vec<String>,
    ) -> Result<(), OAuthQuotaError> {
        self.model_catalog_persistence
            .store(&StoredOAuthModelCatalogSnapshot {
                provider_kind: scope.provider(),
                directory_scope: scope.directory_scope().to_owned(),
                fetched_at: unix_now(),
                models,
            })
            .await
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
}
