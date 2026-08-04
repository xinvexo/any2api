use std::sync::Arc;

use any2api_domain::{OAuthAccountId, ProviderKind};
use any2api_provider::api::{OAuthGrant, OAuthRefreshRejection, OAuthTokenMaterial};
use any2api_storage::api::OAuthAccountDocument;
use thiserror::Error;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::{
    configuration::{ConfigPublishError, PublishedSnapshot},
    oauth::{document, error::OAuthError, login::token_request},
};

use super::worker::OAuthRefresher;

impl OAuthRefresher {
    #[cfg(test)]
    pub(super) async fn refresh_if_due(
        &self,
        id: OAuthAccountId,
        observed_token_version: u64,
    ) -> Result<Option<u64>, OAuthRefreshError> {
        let result = self
            .refresh(id, observed_token_version, RefreshTrigger::Scheduled)
            .await?;
        Ok(match result {
            RefreshResult::Refreshed(snapshot) => snapshot
                .oauth_accounts()
                .get(id)
                .map(|account| account.token_version()),
            RefreshResult::AlreadyUpdated(_)
            | RefreshResult::MissingRefreshToken
            | RefreshResult::PermanentlyRejected
            | RefreshResult::Unavailable => None,
        })
    }

    pub(super) async fn prepare_scheduled_refresh(
        &self,
        id: OAuthAccountId,
        observed_token_version: u64,
    ) -> Result<Option<PreparedOAuthRefresh>, OAuthRefreshError> {
        Ok(
            match self
                .prepare_refresh(id, observed_token_version, RefreshTrigger::Scheduled)
                .await?
            {
                RefreshPreparation::Ready(prepared) => Some(prepared),
                RefreshPreparation::AlreadyUpdated(_)
                | RefreshPreparation::MissingRefreshToken
                | RefreshPreparation::PermanentlyRejected
                | RefreshPreparation::Unavailable => None,
            },
        )
    }

    pub(crate) async fn refresh_after_authentication_failure(
        &self,
        id: OAuthAccountId,
        observed_token_version: u64,
    ) -> OAuthAuthenticationRefreshResult {
        match self
            .refresh(
                id,
                observed_token_version,
                RefreshTrigger::AuthenticationFailure,
            )
            .await
        {
            Ok(RefreshResult::Refreshed(snapshot)) => {
                tracing::info!(oauth_account_id = %id, "OAuth account refreshed after authentication failure");
                OAuthAuthenticationRefreshResult::Refreshed(snapshot)
            }
            Ok(RefreshResult::AlreadyUpdated(snapshot)) => {
                OAuthAuthenticationRefreshResult::Refreshed(snapshot)
            }
            Ok(RefreshResult::MissingRefreshToken) => {
                OAuthAuthenticationRefreshResult::AuthenticationFailed
            }
            Ok(RefreshResult::PermanentlyRejected) => {
                OAuthAuthenticationRefreshResult::AuthenticationFailed
            }
            Err(OAuthRefreshError::RefreshRejected(OAuthRefreshRejection::Permanent)) => {
                OAuthAuthenticationRefreshResult::AuthenticationFailed
            }
            Ok(RefreshResult::Unavailable) => OAuthAuthenticationRefreshResult::Unverified,
            Err(error) => {
                tracing::warn!(
                    oauth_account_id = %id,
                    error = %error,
                    "OAuth account refresh after authentication failure failed"
                );
                OAuthAuthenticationRefreshResult::Unverified
            }
        }
    }

    async fn refresh(
        &self,
        id: OAuthAccountId,
        observed_token_version: u64,
        trigger: RefreshTrigger,
    ) -> Result<RefreshResult, OAuthRefreshError> {
        let prepared = match self
            .prepare_refresh(id, observed_token_version, trigger)
            .await?
        {
            RefreshPreparation::Ready(prepared) => prepared,
            RefreshPreparation::AlreadyUpdated(snapshot) => {
                return Ok(RefreshResult::AlreadyUpdated(snapshot));
            }
            RefreshPreparation::MissingRefreshToken => {
                return Ok(RefreshResult::MissingRefreshToken);
            }
            RefreshPreparation::PermanentlyRejected => {
                return Ok(RefreshResult::PermanentlyRejected);
            }
            RefreshPreparation::Unavailable => return Ok(RefreshResult::Unavailable),
        };
        let (id, observed_token_version, safe_account_email, expires_at, document, _gate) =
            prepared.into_parts();
        let published = match self
            .publisher
            .refresh_oauth_account(
                id,
                observed_token_version,
                safe_account_email,
                expires_at,
                document,
            )
            .await
        {
            Ok(published) => published,
            Err(
                ConfigPublishError::OAuthAccountNotFound
                | ConfigPublishError::OAuthAccountTokenVersionConflict,
            ) => {
                let current = self.publisher.current_snapshot();
                return Ok(match current.oauth_accounts().get(id) {
                    Some(account) if account.token_version() > observed_token_version => {
                        RefreshResult::AlreadyUpdated(current)
                    }
                    _ => RefreshResult::Unavailable,
                });
            }
            Err(error) => return Err(OAuthRefreshError::Publish(error)),
        };
        published
            .oauth_accounts()
            .get(id)
            .filter(|account| account.token_version() > observed_token_version)
            .ok_or(OAuthRefreshError::AccountUnavailable)?;
        Ok(RefreshResult::Refreshed(published))
    }

    async fn prepare_refresh(
        &self,
        id: OAuthAccountId,
        observed_token_version: u64,
        trigger: RefreshTrigger,
    ) -> Result<RefreshPreparation, OAuthRefreshError> {
        let gate = self.gate(id);
        let (guard, waiter_observed_failures) = match Arc::clone(&gate).try_lock_owned() {
            Ok(guard) => (guard, None),
            Err(_) => {
                // Snapshot the failure count before blocking so the waiter
                // can tell afterwards whether the in-flight refresh it
                // queued behind failed its attempt.
                let observed = self.failed_refresh_attempts(id, observed_token_version);
                (gate.lock_owned().await, Some(observed))
            }
        };
        let snapshot = self.publisher.current_snapshot();
        let Some(account) = snapshot.oauth_accounts().get(id) else {
            return Ok(RefreshPreparation::Unavailable);
        };
        let lead_time = snapshot.settings().oauth().refresh_lead_time_secs();
        if account.token_version() != observed_token_version {
            return Ok(if account.token_version() > observed_token_version {
                RefreshPreparation::AlreadyUpdated(snapshot)
            } else {
                RefreshPreparation::Unavailable
            });
        }
        if self.is_permanently_rejected(id, observed_token_version) {
            return Ok(RefreshPreparation::PermanentlyRejected);
        }
        if let Some(observed) = waiter_observed_failures {
            // The refresh this waiter queued behind released the gate without
            // advancing the token version. Exactly one waiter per failed
            // attempt may retry as the new single-flight holder; the rest
            // back off so an IdP outage cannot fan out into a refresh storm.
            if self.failed_refresh_attempts(id, observed_token_version)
                != observed.saturating_add(1)
            {
                return Ok(RefreshPreparation::Unavailable);
            }
        }
        if trigger == RefreshTrigger::Scheduled && !is_due(account.expires_at(), lead_time) {
            return Ok(RefreshPreparation::Unavailable);
        }
        let provider_kind = account.provider_kind();
        let token = snapshot
            .oauth_token_material(id)
            .ok_or(OAuthRefreshError::TokenMaterialUnavailable)?;
        let Some(refresh_token) = token.refresh_token() else {
            tracing::debug!(oauth_account_id = %id, "OAuth account has no refresh token");
            return Ok(RefreshPreparation::MissingRefreshToken);
        };
        match self
            .attempt_refresh(
                id,
                observed_token_version,
                &snapshot,
                provider_kind,
                token.as_ref(),
                refresh_token,
            )
            .await
        {
            Ok((document, safe_account_email, expires_at)) => {
                Ok(RefreshPreparation::Ready(PreparedOAuthRefresh {
                    id,
                    observed_token_version,
                    safe_account_email,
                    expires_at,
                    document,
                    gate: guard,
                }))
            }
            Err(error) => {
                // Record while the gate is still held so a queued waiter
                // observes this failure the moment it wakes.
                self.record_failed_refresh_attempt(id, observed_token_version);
                Err(error)
            }
        }
    }

    /// Executes one refresh round-trip against the identity provider and
    /// parses the result. Any error here counts as a failed attempt for
    /// waiters queued on the account's single-flight gate.
    async fn attempt_refresh(
        &self,
        id: OAuthAccountId,
        observed_token_version: u64,
        snapshot: &PublishedSnapshot,
        provider_kind: ProviderKind,
        token: &OAuthTokenMaterial,
        refresh_token: &str,
    ) -> Result<(OAuthAccountDocument, Option<String>, Option<i64>), OAuthRefreshError> {
        let driver = self
            .providers
            .get(provider_kind)
            .ok_or(OAuthRefreshError::ProviderUnavailable)?;
        let plan = driver
            .oauth_token_request(OAuthGrant::RefreshToken, refresh_token, None, None)
            .map_err(OAuthError::Provider)?;
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthError::PublishedProxyUnavailable)?;
        let strict_ssrf = snapshot.settings().upstream().strict_ssrf();
        let response =
            token_request::execute_response(self.transport.as_ref(), proxy, strict_ssrf, plan)
                .await?;
        if !response.status.is_success() {
            let rejection =
                driver.classify_oauth_refresh_rejection(response.status, &response.body);
            if rejection == OAuthRefreshRejection::Permanent {
                self.record_permanent_rejection(id, observed_token_version);
            }
            return Err(OAuthRefreshError::RefreshRejected(rejection));
        }
        let refreshed = driver
            .parse_oauth_refresh_response(&response.body, token)
            .map_err(OAuthError::from_token_response_error)?;
        if refreshed.provider() != provider_kind {
            return Err(OAuthError::TokenResponseInvalid.into());
        }
        driver
            .oauth_routing_profile(&refreshed)
            .map_err(OAuthError::Provider)?;
        let document = document::build_account_document(&refreshed)?;
        let safe_account_email = refreshed.email().map(str::to_owned);
        let expires_at = refreshed.expires_at();
        Ok((document, safe_account_email, expires_at))
    }

    fn gate(&self, id: OAuthAccountId) -> Arc<Mutex<()>> {
        let mut gates = self.gates.lock().expect("OAuth refresh gate lock poisoned");
        if let Some(gate) = gates.get(&id).and_then(std::sync::Weak::upgrade) {
            return gate;
        }
        let gate = Arc::new(Mutex::new(()));
        gates.insert(id, Arc::downgrade(&gate));
        gate
    }
}

pub(super) struct PreparedOAuthRefresh {
    id: OAuthAccountId,
    observed_token_version: u64,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
    document: OAuthAccountDocument,
    gate: OwnedMutexGuard<()>,
}

type PreparedOAuthRefreshParts = (
    OAuthAccountId,
    u64,
    Option<String>,
    Option<i64>,
    OAuthAccountDocument,
    OwnedMutexGuard<()>,
);

impl PreparedOAuthRefresh {
    pub(super) fn into_parts(self) -> PreparedOAuthRefreshParts {
        (
            self.id,
            self.observed_token_version,
            self.safe_account_email,
            self.expires_at,
            self.document,
            self.gate,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RefreshTrigger {
    Scheduled,
    AuthenticationFailure,
}

enum RefreshResult {
    Refreshed(Arc<PublishedSnapshot>),
    AlreadyUpdated(Arc<PublishedSnapshot>),
    MissingRefreshToken,
    PermanentlyRejected,
    Unavailable,
}

enum RefreshPreparation {
    Ready(PreparedOAuthRefresh),
    AlreadyUpdated(Arc<PublishedSnapshot>),
    MissingRefreshToken,
    PermanentlyRejected,
    Unavailable,
}

pub(crate) enum OAuthAuthenticationRefreshResult {
    Refreshed(Arc<PublishedSnapshot>),
    AuthenticationFailed,
    Unverified,
}

pub(super) fn is_due(expires_at: Option<i64>, lead_time_secs: u64) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };
    let lead_time = i64::try_from(lead_time_secs).expect("validated OAuth lead time fits i64");
    document::unix_now() >= expires_at.saturating_sub(lead_time)
}

#[derive(Debug, Error)]
pub(super) enum OAuthRefreshError {
    #[error("OAuth account disappeared after refresh publication")]
    AccountUnavailable,
    #[error("OAuth provider driver is unavailable")]
    ProviderUnavailable,
    #[error("OAuth token material is unavailable")]
    TokenMaterialUnavailable,
    #[error("OAuth refresh endpoint rejected the token")]
    RefreshRejected(OAuthRefreshRejection),
    #[error(transparent)]
    OAuth(#[from] OAuthError),
    #[error("OAuth refresh publication failed")]
    Publish(#[source] ConfigPublishError),
}
