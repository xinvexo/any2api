use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use any2api_domain::{OAuthAccountId, RoutingCredentialId};
use any2api_provider::api::{
    OAuthQuotaExhaustion, OAuthQuotaResetResult, OAuthQuotaTokenBalance,
    OAuthQuotaTokenBalanceSource, OAuthQuotaUsage, ProviderRegistry,
};
use any2api_storage::api::OAuthQuotaSnapshotRepository;
use any2api_transport::api::TransportManager;
use http::StatusCode;
use tokio::sync::watch;
use uuid::Uuid;

use crate::{
    configuration::{ConfigPublisher, PublishedSnapshot},
    oauth::{
        document,
        refresh::{OAuthAuthenticationRefreshResult, OAuthRefresher},
    },
};

use super::{
    activity::OAuthQuotaActivity,
    health, observation,
    operation_gate::{OAuthQuotaCompletedOperation, OAuthQuotaOperationGates},
    persistence::OAuthQuotaPersistence,
    rejection::RequestContext,
    types::{OAuthQuotaError, OAuthQuotaResetOutcome, OAuthQuotaSnapshot},
};

pub(in crate::oauth) struct OAuthQuotaService {
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
    publisher: Arc<ConfigPublisher>,
    refresher: Arc<OAuthRefresher>,
    persistence: OAuthQuotaPersistence,
    activity: OAuthQuotaActivity,
    operation_gates: OAuthQuotaOperationGates,
}

impl OAuthQuotaService {
    pub(in crate::oauth) fn new(
        providers: Arc<ProviderRegistry>,
        transport: Arc<dyn TransportManager>,
        publisher: Arc<ConfigPublisher>,
        refresher: Arc<OAuthRefresher>,
        quota_repository: Arc<dyn OAuthQuotaSnapshotRepository>,
    ) -> Self {
        Self {
            providers,
            transport,
            publisher,
            refresher,
            persistence: OAuthQuotaPersistence::new(quota_repository),
            activity: OAuthQuotaActivity::new(),
            operation_gates: OAuthQuotaOperationGates::default(),
        }
    }

    pub(in crate::oauth) async fn cached(
        &self,
        id: OAuthAccountId,
    ) -> Result<Option<OAuthQuotaSnapshot>, OAuthQuotaError> {
        if self
            .publisher
            .current_snapshot()
            .oauth_accounts()
            .get(id)
            .is_none()
        {
            return Err(OAuthQuotaError::AccountNotFound);
        }
        self.persistence.load(id).await
    }

    pub(in crate::oauth) async fn refresh(
        &self,
        id: OAuthAccountId,
    ) -> Result<OAuthQuotaSnapshot, OAuthQuotaError> {
        let gate = self.operation_gates.get(id);
        let observed = gate.operation_completed.load(Ordering::Acquire);
        let mut state = gate.state.lock().await;
        if gate.operation_completed.load(Ordering::Acquire) != observed
            && let Some(OAuthQuotaCompletedOperation::Refresh(result)) = &state.last_completed
        {
            return result.as_ref().clone();
        }
        let result = self.refresh_once(id).await;
        state.last_completed = Some(OAuthQuotaCompletedOperation::Refresh(Arc::new(
            result.clone(),
        )));
        gate.operation_completed.fetch_add(1, Ordering::Release);
        result
    }

    async fn refresh_once(
        &self,
        id: OAuthAccountId,
    ) -> Result<OAuthQuotaSnapshot, OAuthQuotaError> {
        let usage = self.query_with_authentication_retry(id).await?;
        let snapshot = OAuthQuotaSnapshot {
            usage,
            fetched_at: document::unix_now(),
        };
        self.persistence.store(id, &snapshot).await?;
        Ok(snapshot)
    }

    pub(in crate::oauth) fn subscribe_changes(&self) -> watch::Receiver<u64> {
        self.persistence.subscribe()
    }

    pub(in crate::oauth) fn activity(&self) -> OAuthQuotaActivity {
        self.activity.clone()
    }

    pub(in crate::oauth) fn start_activity_worker(
        self: &Arc<Self>,
        lifecycle: &crate::lifecycle::ProcessLifecycle,
    ) -> bool {
        self.activity.start(Arc::clone(self), lifecycle)
    }

    pub(in crate::oauth) async fn reset(
        &self,
        id: OAuthAccountId,
        redeem_request_id: Uuid,
    ) -> Result<OAuthQuotaResetOutcome, OAuthQuotaError> {
        let gate = self.operation_gates.get(id);
        let mut state = gate.state.lock().await;
        let result = self.reset_once(id, redeem_request_id).await;
        state.last_completed = Some(OAuthQuotaCompletedOperation::Reset);
        gate.operation_completed.fetch_add(1, Ordering::Release);
        result
    }

    async fn reset_once(
        &self,
        id: OAuthAccountId,
        redeem_request_id: Uuid,
    ) -> Result<OAuthQuotaResetOutcome, OAuthQuotaError> {
        let quota = self.query_with_authentication_retry(id).await?;
        if quota
            .reset_credits
            .as_ref()
            .is_none_or(|credits| credits.available_count == 0)
        {
            return Err(OAuthQuotaError::NoResetCredits);
        }
        let result = self
            .reset_with_authentication_retry(id, &redeem_request_id)
            .await?;
        self.clear_temporary_cooldowns(id);
        self.persistence.delete(id).await?;
        Ok(OAuthQuotaResetOutcome {
            windows_reset: result.windows_reset,
        })
    }

    async fn query_with_authentication_retry(
        &self,
        id: OAuthAccountId,
    ) -> Result<OAuthQuotaUsage, OAuthQuotaError> {
        let snapshot = self.publisher.current_snapshot();
        let observed_token_version = snapshot
            .oauth_accounts()
            .get(id)
            .ok_or(OAuthQuotaError::AccountNotFound)?
            .token_version();
        match self.query_attempt(Arc::clone(&snapshot), id).await {
            Err(OAuthQuotaError::UpstreamRejected(status))
                if status == StatusCode::UNAUTHORIZED.as_u16() =>
            {
                let refresh_result = self
                    .refresher
                    .refresh_after_authentication_failure(id, observed_token_version)
                    .await;
                let refreshed =
                    self.resolve_authentication_refresh(&snapshot, id, refresh_result)?;
                let refreshed_for_health = Arc::clone(&refreshed);
                self.query_attempt(refreshed, id).await.map_err(|error| {
                    self.map_second_authentication_failure(refreshed_for_health.as_ref(), id, error)
                })
            }
            result => result,
        }
    }

    async fn query_attempt(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        id: OAuthAccountId,
    ) -> Result<OAuthQuotaUsage, OAuthQuotaError> {
        let account = snapshot
            .oauth_accounts()
            .get(id)
            .ok_or(OAuthQuotaError::AccountNotFound)?;
        let driver = self
            .providers
            .get(account.provider_kind())
            .ok_or(OAuthQuotaError::ProviderUnavailable)?;
        let binding = snapshot
            .credential_runtime(RoutingCredentialId::oauth_account(id))
            .ok_or(OAuthQuotaError::RuntimeUnavailable)?;
        let generation = binding.generation();
        let token = generation
            .oauth_token()
            .ok_or(OAuthQuotaError::TokenMaterialUnavailable)?;
        let plan = driver
            .oauth_quota_query_plan(token.as_ref())
            .map_err(OAuthQuotaError::Provider)?
            .ok_or(OAuthQuotaError::UnsupportedProvider)?;
        let (usage_plan, supplement_plan, credits_plan) = plan.into_parts();
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthQuotaError::ProxyUnavailable)?;
        let strict_ssrf = snapshot.settings().upstream().strict_ssrf();
        let read_timeout = Duration::from_secs(snapshot.settings().upstream().read_timeout_secs());
        let request = RequestContext::new(
            driver.as_ref(),
            self.transport.as_ref(),
            proxy,
            strict_ssrf,
            read_timeout,
        );
        let usage_response = request.execute(usage_plan).await?;
        if !usage_response.status.is_success() {
            return Err(request.rejection(&usage_response));
        }
        let mut usage =
            observation::resolve_usage(&request, usage_response, supplement_plan).await?;
        health::synchronize(
            generation.health().as_ref(),
            &usage,
            snapshot.reliability_policy().permission_denied,
        );
        let exhaustion = binding
            .generation()
            .health()
            .quota_exhaustion()
            .map(|observed| OAuthQuotaExhaustion {
                observed_at: observed.observed_at,
                used: observed.used,
                limit: observed.limit,
            });
        usage.replace_quota_exhaustion(exhaustion);
        if let Some(balance) = exhaustion.and_then(exhaustion_token_balance) {
            usage.replace_token_balance(Some(balance));
        }
        if let Some(credits_plan) = credits_plan
            && let Ok(response) = request.execute(credits_plan).await
            && response.status.is_success()
            && let Ok(Some(credits)) = driver.parse_oauth_quota_reset_credits(&response.body)
        {
            usage.replace_reset_credits(credits);
        }
        Ok(usage)
    }

    async fn reset_with_authentication_retry(
        &self,
        id: OAuthAccountId,
        redeem_request_id: &Uuid,
    ) -> Result<OAuthQuotaResetResult, OAuthQuotaError> {
        let snapshot = self.publisher.current_snapshot();
        let observed_token_version = snapshot
            .oauth_accounts()
            .get(id)
            .ok_or(OAuthQuotaError::AccountNotFound)?
            .token_version();
        match self
            .reset_attempt(Arc::clone(&snapshot), id, redeem_request_id)
            .await
        {
            Err(OAuthQuotaError::UpstreamRejected(status))
                if status == StatusCode::UNAUTHORIZED.as_u16() =>
            {
                let refresh_result = self
                    .refresher
                    .refresh_after_authentication_failure(id, observed_token_version)
                    .await;
                let refreshed =
                    self.resolve_authentication_refresh(&snapshot, id, refresh_result)?;
                let refreshed_for_health = Arc::clone(&refreshed);
                self.reset_attempt(refreshed, id, redeem_request_id)
                    .await
                    .map_err(|error| {
                        self.map_second_authentication_failure(
                            refreshed_for_health.as_ref(),
                            id,
                            error,
                        )
                    })
            }
            result => result,
        }
    }

    async fn reset_attempt(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        id: OAuthAccountId,
        redeem_request_id: &Uuid,
    ) -> Result<OAuthQuotaResetResult, OAuthQuotaError> {
        let account = snapshot
            .oauth_accounts()
            .get(id)
            .ok_or(OAuthQuotaError::AccountNotFound)?;
        let driver = self
            .providers
            .get(account.provider_kind())
            .ok_or(OAuthQuotaError::ProviderUnavailable)?;
        let binding = snapshot
            .credential_runtime(RoutingCredentialId::oauth_account(id))
            .ok_or(OAuthQuotaError::RuntimeUnavailable)?;
        let token = binding
            .generation()
            .oauth_token()
            .ok_or(OAuthQuotaError::TokenMaterialUnavailable)?;
        let plan = driver
            .oauth_quota_reset_plan(token.as_ref(), &redeem_request_id.to_string())
            .map_err(OAuthQuotaError::Provider)?
            .ok_or(OAuthQuotaError::UnsupportedProvider)?;
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthQuotaError::ProxyUnavailable)?;
        let strict_ssrf = snapshot.settings().upstream().strict_ssrf();
        let read_timeout = Duration::from_secs(snapshot.settings().upstream().read_timeout_secs());
        let request = RequestContext::new(
            driver.as_ref(),
            self.transport.as_ref(),
            proxy,
            strict_ssrf,
            read_timeout,
        );
        let response = request.execute(plan).await?;
        if !response.status.is_success() {
            return Err(request.rejection(&response));
        }
        let result = driver
            .parse_oauth_quota_reset(&response.body)
            .map_err(OAuthQuotaError::Provider)?;
        Ok(result)
    }

    fn clear_temporary_cooldowns(&self, id: OAuthAccountId) {
        if let Some(binding) = self
            .publisher
            .current_snapshot()
            .credential_runtime(RoutingCredentialId::oauth_account(id))
        {
            binding.generation().health().clear_temporary_cooldowns();
        }
    }

    fn resolve_authentication_refresh(
        &self,
        initial_snapshot: &PublishedSnapshot,
        id: OAuthAccountId,
        result: OAuthAuthenticationRefreshResult,
    ) -> Result<Arc<PublishedSnapshot>, OAuthQuotaError> {
        match result {
            OAuthAuthenticationRefreshResult::Refreshed(snapshot) => Ok(snapshot),
            OAuthAuthenticationRefreshResult::AuthenticationFailed => {
                self.mark_authentication_failed(initial_snapshot, id);
                Err(OAuthQuotaError::AuthenticationFailed)
            }
            OAuthAuthenticationRefreshResult::Unverified => {
                Err(OAuthQuotaError::AuthenticationRefreshFailed)
            }
        }
    }

    fn map_second_authentication_failure(
        &self,
        snapshot: &PublishedSnapshot,
        id: OAuthAccountId,
        error: OAuthQuotaError,
    ) -> OAuthQuotaError {
        match error {
            OAuthQuotaError::UpstreamRejected(status)
                if status == StatusCode::UNAUTHORIZED.as_u16() =>
            {
                self.mark_authentication_failed(snapshot, id);
                OAuthQuotaError::AuthenticationFailed
            }
            error => error,
        }
    }

    fn mark_authentication_failed(&self, snapshot: &PublishedSnapshot, id: OAuthAccountId) {
        if let Some(binding) = snapshot.credential_runtime(RoutingCredentialId::oauth_account(id)) {
            binding
                .generation()
                .health()
                .record_authentication_failure();
        }
    }
}

fn exhaustion_token_balance(exhaustion: OAuthQuotaExhaustion) -> Option<OAuthQuotaTokenBalance> {
    exhaustion
        .used
        .zip(exhaustion.limit)
        .map(|(used, limit)| OAuthQuotaTokenBalance {
            source: OAuthQuotaTokenBalanceSource::Upstream,
            used,
            limit,
            remaining: limit.saturating_sub(used),
            window_seconds: None,
        })
}
