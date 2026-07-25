use std::{sync::Arc, time::Instant};

use any2api_domain::{OAuthAccountId, ProviderKind};
use any2api_provider::api::{
    OAuthDeviceTokenPoll, OAuthGrant, OAuthLoginFlow, OAuthRequestPlan, ProviderDriver,
    ProviderRegistry,
};
use any2api_transport::api::TransportManager;
use tokio::sync::Mutex;

use crate::{process_lifecycle::ProcessLifecycle, publisher::ConfigPublisher};

use super::{
    activation, callback,
    error::OAuthError,
    quota::OAuthQuotaService,
    quota_types::{OAuthQuotaError, OAuthQuotaResetOutcome, OAuthQuotaSnapshot},
    refresh::OAuthRefresher,
    session::{
        AuthorizationCodeSession, CALLBACK_SESSION_TTL_SECONDS, DeviceCodeSession,
        OAuthSessionStore,
    },
    token_request,
    types::{OAuthActivationResult, OAuthDevicePollResult, OAuthStartResult},
};

pub struct OAuthService {
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
    publisher: Arc<ConfigPublisher>,
    sessions: Mutex<OAuthSessionStore>,
    refresher: Arc<OAuthRefresher>,
    quota: OAuthQuotaService,
}

impl OAuthService {
    #[must_use]
    pub fn new(
        providers: Arc<ProviderRegistry>,
        transport: Arc<dyn TransportManager>,
        publisher: Arc<ConfigPublisher>,
    ) -> Self {
        let refresher = OAuthRefresher::new(
            Arc::clone(&providers),
            Arc::clone(&transport),
            Arc::clone(&publisher),
        );
        let quota = OAuthQuotaService::new(
            Arc::clone(&providers),
            Arc::clone(&transport),
            Arc::clone(&publisher),
            Arc::clone(&refresher),
        );
        Self {
            providers,
            transport,
            publisher,
            sessions: Mutex::new(OAuthSessionStore::default()),
            refresher,
            quota,
        }
    }

    pub fn start_refresh_worker(&self, lifecycle: &ProcessLifecycle) -> bool {
        self.refresher.start(lifecycle)
    }

    pub(crate) fn refresher(&self) -> Arc<OAuthRefresher> {
        Arc::clone(&self.refresher)
    }

    pub async fn query_quota(
        &self,
        id: OAuthAccountId,
    ) -> Result<OAuthQuotaSnapshot, OAuthQuotaError> {
        self.quota.query(id).await
    }

    pub async fn reset_quota(
        &self,
        id: OAuthAccountId,
    ) -> Result<OAuthQuotaResetOutcome, OAuthQuotaError> {
        self.quota.reset(id).await
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
        self.sessions.lock().await.insert_authorization_code(
            prepared.id,
            prepared.session,
            Instant::now(),
        )?;
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
        let body = self.execute_request(plan).await?;
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
        self.sessions.lock().await.insert_device_code(
            prepared.id,
            prepared.session,
            Instant::now(),
        )?;
        Ok(result)
    }

    pub async fn exchange(
        &self,
        session_id: &str,
        callback_url: &str,
    ) -> Result<OAuthActivationResult, OAuthError> {
        let session = self
            .sessions
            .lock()
            .await
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
        let body = self.execute_request(plan).await?;
        let token = driver
            .parse_oauth_token(&body)
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
        let now = Instant::now();
        let mut session = self
            .sessions
            .lock()
            .await
            .take_device_code(session_id, now)?;
        if let Some(retry_after_seconds) = session.retry_after(now)? {
            self.restore_device_session(session_id, session).await;
            return Ok(OAuthDevicePollResult::Pending {
                retry_after_seconds,
            });
        }
        let driver = self
            .providers
            .get(session.provider)
            .ok_or(OAuthError::ProviderUnavailable)?;
        let plan = driver.oauth_device_token_request(session.device_code())?;
        let response = self.execute_request_with_status(plan).await?;
        let poll = driver
            .parse_oauth_device_token(response.status, &response.body)
            .map_err(OAuthError::from_token_response_error)?;
        match poll {
            OAuthDeviceTokenPoll::Pending => {
                let retry_after_seconds = session.defer(Instant::now(), false)?;
                self.restore_device_session(session_id, session).await;
                Ok(OAuthDevicePollResult::Pending {
                    retry_after_seconds,
                })
            }
            OAuthDeviceTokenPoll::SlowDown => {
                let retry_after_seconds = session.defer(Instant::now(), true)?;
                self.restore_device_session(session_id, session).await;
                Ok(OAuthDevicePollResult::Pending {
                    retry_after_seconds,
                })
            }
            OAuthDeviceTokenPoll::Authorized(token) => activation::publish(
                self.providers.as_ref(),
                self.publisher.as_ref(),
                session.provider,
                token,
            )
            .await
            .map(OAuthDevicePollResult::Complete),
            OAuthDeviceTokenPoll::Denied => Err(OAuthError::AuthorizationDenied),
            OAuthDeviceTokenPoll::Expired => Err(OAuthError::SessionExpired),
        }
    }

    async fn restore_device_session(&self, session_id: &str, session: DeviceCodeSession) {
        self.sessions
            .lock()
            .await
            .restore_device_code(session_id.to_owned(), session);
    }

    async fn execute_request(&self, plan: OAuthRequestPlan) -> Result<bytes::Bytes, OAuthError> {
        let snapshot = self.publisher.current_snapshot();
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthError::PublishedProxyUnavailable)?;
        let strict_ssrf = snapshot.settings().upstream().strict_ssrf();
        token_request::execute(self.transport.as_ref(), proxy, strict_ssrf, plan).await
    }

    async fn execute_request_with_status(
        &self,
        plan: OAuthRequestPlan,
    ) -> Result<token_request::OAuthHttpResponse, OAuthError> {
        let snapshot = self.publisher.current_snapshot();
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthError::PublishedProxyUnavailable)?;
        let strict_ssrf = snapshot.settings().upstream().strict_ssrf();
        token_request::execute_response(self.transport.as_ref(), proxy, strict_ssrf, plan).await
    }
}
