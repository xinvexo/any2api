use std::{sync::Arc, time::Instant};

use any2api_domain::ProviderKind;
use any2api_provider::api::{OAuthGrant, ProviderRegistry, serialize_file};
use any2api_transport::api::TransportManager;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::Mutex;

use super::{
    callback,
    error::OAuthError,
    session::{OAuthSession, OAuthSessionStore, SESSION_TTL_SECONDS},
    token_request,
    types::{OAuthDownload, OAuthStartResult},
};

pub struct OAuthService {
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
    sessions: Mutex<OAuthSessionStore>,
}

impl OAuthService {
    #[must_use]
    pub fn new(providers: Arc<ProviderRegistry>, transport: Arc<dyn TransportManager>) -> Self {
        Self {
            providers,
            transport,
            sessions: Mutex::new(OAuthSessionStore::default()),
        }
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
    ) -> Result<OAuthDownload, OAuthError> {
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
        let body = token_request::execute(self.transport.as_ref(), plan).await?;
        let token = driver
            .parse_oauth_token(&body)
            .map_err(OAuthError::from_token_response_error)?;
        if token.provider() != session.provider {
            return Err(OAuthError::TokenResponseInvalid);
        }
        let now = unix_now();
        let last_refresh = format_timestamp(now)?;
        let expired = token
            .expires_at()
            .map(format_timestamp)
            .transpose()?
            .unwrap_or_default();
        let bytes = serialize_file(&token, &last_refresh, &expired)
            .map_err(|_| OAuthError::FileSerialization)?;
        Ok(OAuthDownload::new(session.provider, bytes))
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
}

fn format_timestamp(timestamp: i64) -> Result<String, OAuthError> {
    let value = OffsetDateTime::from_unix_timestamp(timestamp)
        .map_err(|_| OAuthError::FileSerialization)?;
    value
        .format(&Rfc3339)
        .map_err(|_| OAuthError::FileSerialization)
}
