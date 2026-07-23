use std::{sync::Arc, time::Instant};

use any2api_domain::{CredentialId, CredentialKind, ProviderEndpointId};
use any2api_provider::api::{OAuthGrant, ProviderRegistry};
use any2api_transport::api::TransportManager;
use tokio::sync::Mutex;

use crate::{
    process_lifecycle::ProcessLifecycle,
    provider_oauth2_secret::ProviderOAuth2Secret,
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    publisher::ConfigPublisher,
};

use super::{
    callback,
    error::ProviderOAuthError,
    refresh,
    session::{OAuthSession, OAuthSessionStore, SESSION_TTL_SECONDS},
    token_request,
    types::{ProviderOAuthExchangeResult, ProviderOAuthStartRequest, ProviderOAuthStartResult},
};

pub struct ProviderOAuthService {
    pub(super) providers: Arc<ProviderRegistry>,
    pub(super) transport: Arc<dyn TransportManager>,
    pub(super) snapshots: Arc<SnapshotStore>,
    pub(super) publisher: Arc<ConfigPublisher>,
    sessions: Mutex<OAuthSessionStore>,
}

impl ProviderOAuthService {
    #[must_use]
    pub fn new(
        providers: Arc<ProviderRegistry>,
        transport: Arc<dyn TransportManager>,
        snapshots: Arc<SnapshotStore>,
        publisher: Arc<ConfigPublisher>,
    ) -> Self {
        Self {
            providers,
            transport,
            snapshots,
            publisher,
            sessions: Mutex::new(OAuthSessionStore::default()),
        }
    }

    pub fn start_refresh_worker(self: &Arc<Self>, lifecycle: &ProcessLifecycle) {
        let service = Arc::clone(self);
        lifecycle.spawn_until_draining(async move { refresh::run(service).await });
    }

    #[cfg(test)]
    pub(crate) async fn refresh_credential_for_test(
        &self,
        credential_id: CredentialId,
    ) -> Result<(), ProviderOAuthError> {
        refresh::refresh_one(self, credential_id).await
    }

    pub async fn start(
        &self,
        endpoint_id: ProviderEndpointId,
        request: ProviderOAuthStartRequest,
    ) -> Result<ProviderOAuthStartResult, ProviderOAuthError> {
        let snapshot = self.snapshots.load();
        if snapshot.revision() != request.expected_revision {
            return Err(ProviderOAuthError::RevisionConflict {
                expected: request.expected_revision,
                actual: snapshot.revision(),
            });
        }
        let endpoint = snapshot
            .provider_endpoints()
            .get(endpoint_id)
            .ok_or(ProviderOAuthError::ProviderEndpointNotFound)?;
        let driver = self
            .providers
            .get(endpoint.provider_kind())
            .ok_or(ProviderOAuthError::ProviderUnavailable)?;
        if !driver
            .capabilities()
            .credential_kinds
            .contains(&CredentialKind::OAuth2)
        {
            return Err(ProviderOAuthError::OAuthUnsupported(
                endpoint.provider_kind(),
            ));
        }
        let redirect_uri =
            driver
                .oauth_redirect_uri()
                .ok_or(ProviderOAuthError::OAuthUnsupported(
                    endpoint.provider_kind(),
                ))?;
        let resolved_proxy = snapshot
            .resolved_transport_proxy_for_profile(request.draft.proxy_profile_id())
            .ok_or(ProviderOAuthError::ProxyNotFound)?;
        if !resolved_proxy.profile().enabled() {
            return Err(ProviderOAuthError::ProxyDisabled);
        }
        let prepared = OAuthSession::prepare(
            endpoint.provider_kind(),
            endpoint_id,
            endpoint.config_version(),
            resolved_proxy.profile().id(),
            resolved_proxy.profile().config_version(),
            request.draft,
            redirect_uri,
            Instant::now(),
        )?;
        let authorization_url = driver
            .oauth_authorization_url(&prepared.state, &prepared.code_challenge)?
            .to_string();
        let session_id = prepared.id.clone();
        self.sessions
            .lock()
            .await
            .insert(prepared.id, prepared.session, Instant::now())?;
        Ok(ProviderOAuthStartResult::new(
            session_id,
            authorization_url,
            redirect_uri,
            SESSION_TTL_SECONDS,
        ))
    }

    pub async fn exchange(
        &self,
        endpoint_id: ProviderEndpointId,
        session_id: &str,
        callback_url: &str,
    ) -> Result<ProviderOAuthExchangeResult, ProviderOAuthError> {
        let session = self
            .sessions
            .lock()
            .await
            .take(session_id, Instant::now())?;
        if session.endpoint_id != endpoint_id {
            return Err(ProviderOAuthError::InvalidSession);
        }
        let callback = callback::parse(callback_url, session.redirect_uri, session.state())?;
        let snapshot = self.snapshots.load();
        self.validate_session_configuration(&snapshot, &session)?;
        let driver = self
            .providers
            .get(session.provider_kind)
            .ok_or(ProviderOAuthError::ProviderUnavailable)?;
        let plan = driver.oauth_token_request(
            OAuthGrant::AuthorizationCode,
            &callback.code,
            Some(session.state()),
            Some(session.code_verifier()),
        )?;
        let body = token_request::execute(
            self.transport.as_ref(),
            snapshot.as_ref(),
            session.draft.proxy_profile_id(),
            plan,
        )
        .await?;
        let token = driver
            .parse_oauth_token(&body, None)
            .map_err(ProviderOAuthError::from_token_response_error)?;
        let oauth_secret = ProviderOAuth2Secret::from_token(&token)?;
        let latest = self.snapshots.load();
        self.validate_session_configuration(&latest, &session)?;
        let credential_id = CredentialId::new();
        let published = self
            .publisher
            .create_provider_oauth_credential(
                credential_id,
                session.endpoint_id,
                session.endpoint_config_version,
                session.draft,
                oauth_secret,
            )
            .await?;
        Ok(ProviderOAuthExchangeResult::new(
            published.revision(),
            session.endpoint_id,
            credential_id,
            session.provider_kind,
            token.account_id().map(str::to_owned),
            token.email().map(str::to_owned),
            token.organization_id().map(str::to_owned),
        ))
    }

    fn validate_session_configuration(
        &self,
        snapshot: &PublishedSnapshot,
        session: &OAuthSession,
    ) -> Result<(), ProviderOAuthError> {
        let endpoint = snapshot
            .provider_endpoints()
            .get(session.endpoint_id)
            .ok_or(ProviderOAuthError::ConfigurationChanged)?;
        if endpoint.provider_kind() != session.provider_kind
            || endpoint.config_version() != session.endpoint_config_version
        {
            return Err(ProviderOAuthError::ConfigurationChanged);
        }
        let proxy = snapshot
            .resolved_transport_proxy_for_profile(session.draft.proxy_profile_id())
            .ok_or(ProviderOAuthError::ConfigurationChanged)?;
        if !proxy.profile().enabled()
            || proxy.profile().id() != session.resolved_proxy_id
            || proxy.profile().config_version() != session.resolved_proxy_config_version
        {
            return Err(ProviderOAuthError::ConfigurationChanged);
        }
        Ok(())
    }
}
