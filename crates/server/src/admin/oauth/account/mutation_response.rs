use any2api_runtime::api::{OAuthService, PublishedSnapshot};
use serde::Serialize;

use super::dto::{OAuthAccountCoreResponse, accounts_for_management};

#[derive(Debug, Serialize)]
pub(super) struct OAuthAccountMutationResponse {
    config_revision: u64,
    items: Vec<OAuthAccountCoreResponse>,
}

impl OAuthAccountMutationResponse {
    pub(super) fn from_snapshot(
        snapshot: &PublishedSnapshot,
        oauth: Option<&OAuthService>,
    ) -> Self {
        Self {
            config_revision: snapshot.revision().get(),
            items: accounts_for_management(snapshot.oauth_accounts().accounts())
                .into_iter()
                .map(|account| OAuthAccountCoreResponse::from_snapshot(account, snapshot, oauth))
                .collect(),
        }
    }
}
