use std::sync::Arc;

use any2api_domain::{OAuthAccountId, RoutingCredentialId};
use http::StatusCode;

use crate::{configuration::PublishedSnapshot, oauth::refresh::OAuthAuthenticationRefreshResult};

use super::{coordinator::OAuthQuotaService, types::OAuthQuotaError};

impl OAuthQuotaService {
    pub(super) fn resolve_authentication_refresh(
        &self,
        initial_snapshot: &PublishedSnapshot,
        id: OAuthAccountId,
        result: OAuthAuthenticationRefreshResult,
    ) -> Result<Arc<PublishedSnapshot>, OAuthQuotaError> {
        match result {
            OAuthAuthenticationRefreshResult::Refreshed(snapshot) => Ok(snapshot),
            OAuthAuthenticationRefreshResult::MissingRefreshToken(failure) => {
                self.mark_authentication_failed(initial_snapshot, id);
                Err(OAuthQuotaError::RefreshTokenMissing(failure))
            }
            OAuthAuthenticationRefreshResult::PermanentlyRejected(failure) => {
                self.mark_authentication_failed(initial_snapshot, id);
                Err(OAuthQuotaError::RefreshPermanentlyRejected(failure))
            }
            OAuthAuthenticationRefreshResult::Failed(failure) => {
                Err(OAuthQuotaError::TokenRefreshFailed(failure))
            }
        }
    }

    pub(super) fn map_second_authentication_failure(
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
                let Some(account) = snapshot.oauth_accounts().get(id) else {
                    return OAuthQuotaError::AccountNotFound;
                };
                let failure = self
                    .refresher
                    .record_refreshed_access_token_rejected(id, account.token_version());
                OAuthQuotaError::RefreshedAccessTokenRejected(failure)
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
