use std::{sync::Arc, time::Instant};

use any2api_domain::{MaxConcurrency, OAuthAccountDraft, OAuthAccountId, ProviderKind};
use any2api_provider::api::{OAuthGrant, ProviderRegistry};
use any2api_transport::api::TransportManager;
use tokio::sync::Mutex;

use crate::{process_lifecycle::ProcessLifecycle, publisher::ConfigPublisher};

use super::{
    callback, document,
    error::OAuthError,
    refresh::OAuthRefresher,
    session::{OAuthSession, OAuthSessionStore, SESSION_TTL_SECONDS},
    token_request,
    types::{OAuthActivationResult, OAuthStartResult},
};

pub struct OAuthService {
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
    publisher: Arc<ConfigPublisher>,
    sessions: Mutex<OAuthSessionStore>,
    refresher: Arc<OAuthRefresher>,
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
        Self {
            providers,
            transport,
            publisher,
            sessions: Mutex::new(OAuthSessionStore::default()),
            refresher,
        }
    }

    pub fn start_refresh_worker(&self, lifecycle: &ProcessLifecycle) -> bool {
        self.refresher.start(lifecycle)
    }

    pub(crate) fn refresher(&self) -> Arc<OAuthRefresher> {
        Arc::clone(&self.refresher)
    }

    pub async fn start(&self, provider: ProviderKind) -> Result<OAuthStartResult, OAuthError> {
        let driver = self
            .providers
            .get(provider)
            .ok_or(OAuthError::ProviderUnavailable)?;
        let redirect_uri = driver
            .oauth_redirect_uri()
            .ok_or(OAuthError::UnsupportedProvider(provider))?;
        let prepared = OAuthSession::prepare(provider, redirect_uri, Instant::now())?;
        let authorization_url = driver
            .oauth_authorization_url(&prepared.state, &prepared.code_challenge)?
            .to_string();
        let session_id = prepared.id.clone();
        self.sessions
            .lock()
            .await
            .insert(prepared.id, prepared.session, Instant::now())?;
        Ok(OAuthStartResult::new(
            provider,
            session_id,
            authorization_url,
            redirect_uri,
            SESSION_TTL_SECONDS,
        ))
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
            .take(session_id, Instant::now())?;
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
        let exchange_snapshot = self.publisher.current_snapshot();
        let proxy = exchange_snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthError::PublishedProxyUnavailable)?;
        let strict_ssrf = exchange_snapshot.settings().upstream().strict_ssrf();
        let body =
            token_request::execute(self.transport.as_ref(), proxy, strict_ssrf, plan).await?;
        let token = driver
            .parse_oauth_token(&body)
            .map_err(OAuthError::from_token_response_error)?;
        if token.provider() != session.provider {
            return Err(OAuthError::TokenResponseInvalid);
        }
        let routing_profile = driver.oauth_routing_profile(&token)?;
        let models = routing_profile
            .models()
            .iter()
            .map(|model| model.as_str().to_owned())
            .collect();
        let document = document::serialize(&token)?;
        let account_id = OAuthAccountId::new();
        let draft = OAuthAccountDraft::new(
            default_label(session.provider, account_id),
            MaxConcurrency::new(1).expect("OAuth default concurrency is valid"),
            true,
        )
        .map_err(|_| OAuthError::DocumentSerialization)?;
        let published = self
            .publisher
            .activate_oauth_account(
                account_id,
                session.provider,
                draft,
                token.email().map(str::to_owned),
                token.expires_at(),
                models,
                document,
            )
            .await
            .map_err(OAuthError::Activation)?;
        let account = published
            .oauth_accounts()
            .get(account_id)
            .cloned()
            .expect("published OAuth account is present after activation");
        Ok(OAuthActivationResult::new(published.revision(), account))
    }
}

fn default_label(provider: ProviderKind, account_id: OAuthAccountId) -> String {
    let provider = match provider {
        ProviderKind::Codex => "Codex",
        ProviderKind::Claude => "Claude",
    };
    format!("{provider} OAuth {account_id}")
}
