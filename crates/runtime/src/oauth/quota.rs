use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex, Weak},
    time::Duration,
};

use any2api_domain::{OAuthAccountId, RoutingCredentialId};
use any2api_provider::api::{OAuthQuotaResetResult, OAuthQuotaUsage, ProviderRegistry};
use any2api_transport::api::TransportManager;
use http::StatusCode;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{published_snapshot::PublishedSnapshot, publisher::ConfigPublisher};

use super::{
    document, quota_request,
    quota_types::{OAuthQuotaError, OAuthQuotaResetOutcome, OAuthQuotaSnapshot},
    refresh::OAuthRefresher,
};

pub(super) struct OAuthQuotaService {
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
    publisher: Arc<ConfigPublisher>,
    refresher: Arc<OAuthRefresher>,
    reset_gates: StdMutex<HashMap<OAuthAccountId, Weak<Mutex<()>>>>,
}

impl OAuthQuotaService {
    pub(super) fn new(
        providers: Arc<ProviderRegistry>,
        transport: Arc<dyn TransportManager>,
        publisher: Arc<ConfigPublisher>,
        refresher: Arc<OAuthRefresher>,
    ) -> Self {
        Self {
            providers,
            transport,
            publisher,
            refresher,
            reset_gates: StdMutex::new(HashMap::new()),
        }
    }

    pub(super) async fn query(
        &self,
        id: OAuthAccountId,
    ) -> Result<OAuthQuotaSnapshot, OAuthQuotaError> {
        let usage = self.query_with_authentication_retry(id).await?;
        Ok(OAuthQuotaSnapshot {
            usage,
            fetched_at: document::unix_now(),
        })
    }

    pub(super) async fn reset(
        &self,
        id: OAuthAccountId,
    ) -> Result<OAuthQuotaResetOutcome, OAuthQuotaError> {
        let gate = self.reset_gate(id);
        let _guard = gate.lock().await;
        let quota = self.query_with_authentication_retry(id).await?;
        if quota
            .reset_credits
            .as_ref()
            .is_none_or(|credits| credits.available_count == 0)
        {
            return Err(OAuthQuotaError::NoResetCredits);
        }
        let result = self.reset_with_authentication_retry(id).await?;
        self.clear_temporary_cooldowns(id);
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
                let refreshed = self
                    .refresher
                    .refresh_after_authentication_failure(id, observed_token_version)
                    .await
                    .ok_or(OAuthQuotaError::AuthenticationFailed)?;
                self.query_attempt(refreshed, id)
                    .await
                    .map_err(map_second_authentication_failure)
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
        let permit = binding
            .try_acquire()
            .ok_or(OAuthQuotaError::CredentialAtCapacity)?;
        let token = permit
            .generation()
            .oauth_token()
            .ok_or(OAuthQuotaError::TokenMaterialUnavailable)?;
        let plan = driver
            .oauth_quota_query_plan(token.as_ref())
            .map_err(OAuthQuotaError::Provider)?
            .ok_or(OAuthQuotaError::UnsupportedProvider)?;
        let (usage_plan, credits_plan) = plan.into_parts();
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthQuotaError::ProxyUnavailable)?;
        let strict_ssrf = snapshot.settings().upstream().strict_ssrf();
        let read_timeout = Duration::from_secs(snapshot.settings().upstream().read_timeout_secs());
        let usage_response = quota_request::execute(
            self.transport.as_ref(),
            proxy,
            strict_ssrf,
            read_timeout,
            usage_plan,
        )
        .await?;
        if !usage_response.status.is_success() {
            return Err(OAuthQuotaError::UpstreamRejected(
                usage_response.status.as_u16(),
            ));
        }
        let mut usage = driver
            .parse_oauth_quota_usage(&usage_response.body)
            .map_err(OAuthQuotaError::Provider)?;
        if let Ok(response) = quota_request::execute(
            self.transport.as_ref(),
            proxy,
            strict_ssrf,
            read_timeout,
            credits_plan,
        )
        .await
            && response.status.is_success()
            && let Ok(Some(credits)) = driver.parse_oauth_quota_reset_credits(&response.body)
        {
            usage.replace_reset_credits(credits);
        }
        drop(permit);
        Ok(usage)
    }

    async fn reset_with_authentication_retry(
        &self,
        id: OAuthAccountId,
    ) -> Result<OAuthQuotaResetResult, OAuthQuotaError> {
        let snapshot = self.publisher.current_snapshot();
        let observed_token_version = snapshot
            .oauth_accounts()
            .get(id)
            .ok_or(OAuthQuotaError::AccountNotFound)?
            .token_version();
        match self.reset_attempt(Arc::clone(&snapshot), id).await {
            Err(OAuthQuotaError::UpstreamRejected(status))
                if status == StatusCode::UNAUTHORIZED.as_u16() =>
            {
                let refreshed = self
                    .refresher
                    .refresh_after_authentication_failure(id, observed_token_version)
                    .await
                    .ok_or(OAuthQuotaError::AuthenticationFailed)?;
                self.reset_attempt(refreshed, id)
                    .await
                    .map_err(map_second_authentication_failure)
            }
            result => result,
        }
    }

    async fn reset_attempt(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        id: OAuthAccountId,
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
        let permit = binding
            .try_acquire()
            .ok_or(OAuthQuotaError::CredentialAtCapacity)?;
        let token = permit
            .generation()
            .oauth_token()
            .ok_or(OAuthQuotaError::TokenMaterialUnavailable)?;
        let plan = driver
            .oauth_quota_reset_plan(token.as_ref(), &Uuid::new_v4().to_string())
            .map_err(OAuthQuotaError::Provider)?
            .ok_or(OAuthQuotaError::UnsupportedProvider)?;
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthQuotaError::ProxyUnavailable)?;
        let response = quota_request::execute(
            self.transport.as_ref(),
            proxy,
            snapshot.settings().upstream().strict_ssrf(),
            Duration::from_secs(snapshot.settings().upstream().read_timeout_secs()),
            plan,
        )
        .await?;
        if !response.status.is_success() {
            return Err(OAuthQuotaError::UpstreamRejected(response.status.as_u16()));
        }
        let result = driver
            .parse_oauth_quota_reset(&response.body)
            .map_err(OAuthQuotaError::Provider)?;
        drop(permit);
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

    fn reset_gate(&self, id: OAuthAccountId) -> Arc<Mutex<()>> {
        let mut gates = self
            .reset_gates
            .lock()
            .expect("OAuth quota reset gate lock poisoned");
        gates.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = gates.get(&id).and_then(Weak::upgrade) {
            return gate;
        }
        let gate = Arc::new(Mutex::new(()));
        gates.insert(id, Arc::downgrade(&gate));
        gate
    }
}

fn map_second_authentication_failure(error: OAuthQuotaError) -> OAuthQuotaError {
    match error {
        OAuthQuotaError::UpstreamRejected(status)
            if status == StatusCode::UNAUTHORIZED.as_u16() =>
        {
            OAuthQuotaError::AuthenticationFailed
        }
        error => error,
    }
}
