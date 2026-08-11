use std::{sync::Arc, time::Instant};

use any2api_domain::{OAuthAccountId, ProviderKind};
use any2api_provider::api::{
    OAuthDeviceTokenPoll, OAuthGrant, OAuthLoginFlow, OAuthRequestPlan, ProviderDriver,
    ProviderRegistry,
};
use any2api_storage::api::{OAuthQuotaEstimationRepository, OAuthQuotaSnapshotRepository};
use any2api_transport::api::{TransportIsolationKey, TransportManager, TransportTrafficClass};
use tokio::sync::watch;

use crate::{
    configuration::ConfigPublisher, lifecycle::ProcessLifecycle,
    request_telemetry::RequestTelemetry,
};

use super::{
    control_plane::{OAUTH_CONTROL_PLANE_MIN_START_INTERVAL, OAuthControlPlanePacer},
    error::OAuthError,
    import::{self, OAuthImportError, OAuthImportResult},
    login::{
        OAuthActivationResult, OAuthDevicePollResult, OAuthStartResult, activation, callback,
        session::{
            AuthorizationCodeSession, CALLBACK_SESSION_TTL_SECONDS, DeviceCodeSession,
            DevicePollAcquisition, OAuthSessionRegistry,
        },
        token_request,
    },
    quota::{OAuthQuotaError, OAuthQuotaResetOutcome, OAuthQuotaService, OAuthQuotaSnapshot},
    refresh::{OAuthRefreshFailure, OAuthRefresher},
};

pub struct OAuthService {
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
    publisher: Arc<ConfigPublisher>,
    control_plane: Arc<OAuthControlPlanePacer>,
    sessions: OAuthSessionRegistry,
    refresher: Arc<OAuthRefresher>,
    quota: Arc<OAuthQuotaService>,
}

impl OAuthService {
    #[must_use]
    pub fn new<R>(
        providers: Arc<ProviderRegistry>,
        transport: Arc<dyn TransportManager>,
        publisher: Arc<ConfigPublisher>,
        quota_repository: Arc<R>,
        telemetry: Arc<RequestTelemetry>,
    ) -> Self
    where
        R: OAuthQuotaEstimationRepository + OAuthQuotaSnapshotRepository + 'static,
    {
        Self::new_with_control_plane_interval(
            providers,
            transport,
            publisher,
            quota_repository,
            telemetry,
            OAUTH_CONTROL_PLANE_MIN_START_INTERVAL,
        )
    }

    fn new_with_control_plane_interval<R>(
        providers: Arc<ProviderRegistry>,
        transport: Arc<dyn TransportManager>,
        publisher: Arc<ConfigPublisher>,
        quota_repository: Arc<R>,
        telemetry: Arc<RequestTelemetry>,
        control_plane_interval: std::time::Duration,
    ) -> Self
    where
        R: OAuthQuotaEstimationRepository + OAuthQuotaSnapshotRepository + 'static,
    {
        let control_plane = Arc::new(OAuthControlPlanePacer::new(control_plane_interval));
        let refresher = OAuthRefresher::new(
            Arc::clone(&providers),
            Arc::clone(&transport),
            Arc::clone(&publisher),
            Arc::clone(&control_plane),
        );
        let quota = Arc::new(OAuthQuotaService::new(
            Arc::clone(&providers),
            Arc::clone(&transport),
            Arc::clone(&publisher),
            Arc::clone(&refresher),
            Arc::clone(&control_plane),
            quota_repository,
            telemetry,
        ));
        Self {
            providers,
            transport,
            publisher,
            control_plane,
            sessions: OAuthSessionRegistry::default(),
            refresher,
            quota,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test<R>(
        providers: Arc<ProviderRegistry>,
        transport: Arc<dyn TransportManager>,
        publisher: Arc<ConfigPublisher>,
        quota_repository: Arc<R>,
    ) -> Self
    where
        R: OAuthQuotaEstimationRepository + OAuthQuotaSnapshotRepository + 'static,
    {
        Self::new_with_control_plane_interval(
            providers,
            transport,
            publisher,
            quota_repository,
            Arc::new(RequestTelemetry::disabled()),
            std::time::Duration::ZERO,
        )
    }

    pub fn start_refresh_worker(&self, lifecycle: &ProcessLifecycle) -> bool {
        self.refresher.start(lifecycle)
    }

    pub fn start_quota_worker(&self, lifecycle: &ProcessLifecycle) -> bool {
        self.quota.start_activity_worker(lifecycle)
    }

    pub(crate) fn refresher(&self) -> Arc<OAuthRefresher> {
        Arc::clone(&self.refresher)
    }

    pub(crate) fn quota_activity(&self) -> super::quota::OAuthQuotaActivity {
        self.quota.activity()
    }

    pub async fn cached_quota(
        &self,
        id: OAuthAccountId,
    ) -> Result<Option<OAuthQuotaSnapshot>, OAuthQuotaError> {
        self.quota.cached(id).await
    }

    pub async fn refresh_quota(
        &self,
        id: OAuthAccountId,
    ) -> Result<OAuthQuotaSnapshot, OAuthQuotaError> {
        self.quota.refresh(id).await
    }

    pub fn subscribe_quota_changes(&self) -> watch::Receiver<u64> {
        self.quota.subscribe_changes()
    }

    #[must_use]
    pub fn refresh_failure(
        &self,
        id: OAuthAccountId,
        token_version: u64,
    ) -> Option<OAuthRefreshFailure> {
        self.refresher.refresh_failure(id, token_version)
    }

    pub fn subscribe_refresh_failure_changes(&self) -> watch::Receiver<u64> {
        self.refresher.subscribe_refresh_failure_changes()
    }

    pub async fn reset_quota(
        &self,
        id: OAuthAccountId,
        redeem_request_id: uuid::Uuid,
    ) -> Result<OAuthQuotaResetOutcome, OAuthQuotaError> {
        self.quota.reset(id, redeem_request_id).await
    }

    pub async fn import_files(
        &self,
        files: Vec<bytes::Bytes>,
    ) -> Result<OAuthImportResult, OAuthImportError> {
        import::publish(self.providers.as_ref(), self.publisher.as_ref(), files).await
    }

    pub async fn start(&self, provider: ProviderKind) -> Result<OAuthStartResult, OAuthError> {
        let driver = self
            .providers
            .get(provider)
            .ok_or(OAuthError::ProviderUnavailable)?;
        match driver
            .oauth_login_flow()
            .ok_or(OAuthError::UnsupportedProvider(provider))?
        {
            OAuthLoginFlow::AuthorizationCodePkce => {
                self.start_authorization_code(provider, driver.as_ref())
                    .await
            }
            OAuthLoginFlow::DeviceCode => self.start_device_code(provider, driver.as_ref()).await,
        }
    }

    async fn start_authorization_code(
        &self,
        provider: ProviderKind,
        driver: &dyn ProviderDriver,
    ) -> Result<OAuthStartResult, OAuthError> {
        let redirect_uri = driver
            .oauth_redirect_uri()
            .ok_or(OAuthError::UnsupportedProvider(provider))?;
        let prepared = AuthorizationCodeSession::prepare(provider, redirect_uri, Instant::now())?;
        let authorization_url = driver
            .oauth_authorization_url(&prepared.state, &prepared.code_challenge)?
            .to_string();
        let session_id = prepared.id.clone();
        self.sessions
            .insert_authorization_code(prepared.id, prepared.session, Instant::now())?;
        Ok(OAuthStartResult::authorization_code(
            provider,
            session_id,
            authorization_url,
            redirect_uri,
            CALLBACK_SESSION_TTL_SECONDS,
        ))
    }

    async fn start_device_code(
        &self,
        provider: ProviderKind,
        driver: &dyn ProviderDriver,
    ) -> Result<OAuthStartResult, OAuthError> {
        let plan = driver.oauth_device_authorization_request()?;
        let body = self.execute_request(provider, plan).await?;
        let authorization = driver
            .parse_oauth_device_authorization(&body)
            .map_err(OAuthError::from_token_response_error)?;
        let now = Instant::now();
        let prepared = DeviceCodeSession::prepare(
            provider,
            authorization.device_code(),
            authorization.expires_in_seconds(),
            authorization.poll_interval_seconds(),
            now,
        )?;
        let result = OAuthStartResult::device_code(
            provider,
            prepared.id.clone(),
            authorization.user_code().to_owned(),
            authorization.verification_uri().to_string(),
            authorization
                .verification_uri_complete()
                .map(ToString::to_string),
            prepared.expires_in_seconds,
            prepared.poll_interval_seconds,
        );
        self.sessions
            .insert_device_code(prepared.id, prepared.session, Instant::now())?;
        Ok(result)
    }

    pub async fn exchange(
        &self,
        session_id: &str,
        callback_url: &str,
    ) -> Result<OAuthActivationResult, OAuthError> {
        let session = self
            .sessions
            .take_authorization_code(session_id, Instant::now())?;
        let callback = callback::parse(callback_url, session.redirect_uri, session.state())?;
        let driver = self
            .providers
            .get(session.provider)
            .ok_or(OAuthError::ProviderUnavailable)?;
        let plan = driver.oauth_token_request(
            OAuthGrant::AuthorizationCode,
            &callback.code,
            Some(session.state()),
            Some(session.code_verifier()),
        )?;
        let body = self.execute_request(session.provider, plan).await?;
        let token = driver
            .parse_oauth_token_response(&body)
            .map_err(OAuthError::from_token_response_error)?;
        activation::publish(
            self.providers.as_ref(),
            self.publisher.as_ref(),
            session.provider,
            token,
        )
        .await
    }

    pub async fn poll_device(&self, session_id: &str) -> Result<OAuthDevicePollResult, OAuthError> {
        let lease = match self
            .sessions
            .acquire_device_poll(session_id, Instant::now())?
        {
            DevicePollAcquisition::Ready(lease) => lease,
            DevicePollAcquisition::Pending {
                retry_after_seconds,
            } => {
                return Ok(OAuthDevicePollResult::Pending {
                    retry_after_seconds,
                });
            }
        };
        let driver = self
            .providers
            .get(lease.provider())
            .ok_or(OAuthError::ProviderUnavailable)?;
        let plan = driver.oauth_device_token_request(lease.device_code())?;
        let response = self
            .execute_request_with_status(lease.provider(), plan)
            .await?;
        let poll = driver
            .parse_oauth_device_token(response.status, &response.body)
            .map_err(OAuthError::from_token_response_error)?;
        match poll {
            OAuthDeviceTokenPoll::Pending => {
                let retry_after_seconds = lease.restore(false)?;
                Ok(OAuthDevicePollResult::Pending {
                    retry_after_seconds,
                })
            }
            OAuthDeviceTokenPoll::SlowDown => {
                let retry_after_seconds = lease.restore(true)?;
                Ok(OAuthDevicePollResult::Pending {
                    retry_after_seconds,
                })
            }
            OAuthDeviceTokenPoll::Authorized(token) => {
                let provider = lease.provider();
                lease.finish();
                activation::publish(
                    self.providers.as_ref(),
                    self.publisher.as_ref(),
                    provider,
                    token,
                )
                .await
                .map(OAuthDevicePollResult::Complete)
            }
            OAuthDeviceTokenPoll::Denied => {
                lease.finish();
                Err(OAuthError::AuthorizationDenied)
            }
            OAuthDeviceTokenPoll::Expired => {
                lease.finish();
                Err(OAuthError::SessionExpired)
            }
        }
    }

    async fn execute_request(
        &self,
        provider: ProviderKind,
        plan: OAuthRequestPlan,
    ) -> Result<bytes::Bytes, OAuthError> {
        let snapshot = self.publisher.current_snapshot();
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthError::PublishedProxyUnavailable)?;
        let strict_ssrf = snapshot.settings().upstream().strict_ssrf();
        token_request::execute(
            self.transport.as_ref(),
            self.control_plane.as_ref(),
            provider,
            proxy,
            strict_ssrf,
            TransportIsolationKey::ephemeral(TransportTrafficClass::OAuthToken),
            plan,
        )
        .await
    }

    async fn execute_request_with_status(
        &self,
        provider: ProviderKind,
        plan: OAuthRequestPlan,
    ) -> Result<token_request::OAuthHttpResponse, OAuthError> {
        let snapshot = self.publisher.current_snapshot();
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthError::PublishedProxyUnavailable)?;
        let strict_ssrf = snapshot.settings().upstream().strict_ssrf();
        token_request::execute_response(
            self.transport.as_ref(),
            self.control_plane.as_ref(),
            provider,
            proxy,
            strict_ssrf,
            TransportIsolationKey::ephemeral(TransportTrafficClass::OAuthToken),
            plan,
        )
        .await
    }
}
