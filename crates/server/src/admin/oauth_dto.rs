use any2api_domain::{
    ConfigRevision, MaxConcurrency, OAuthAccount, OAuthAccountDraft, OAuthAccountId, ProviderKind,
};
use any2api_runtime::api::{OAuthActivationResult, OAuthStartResult, PublishedSnapshot};
use serde::{Deserialize, Serialize};

use super::{error::AdminApiError, revision::parse_revision};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthStartRequest {
    provider: ProviderKind,
}

impl OAuthStartRequest {
    pub(super) const fn provider(&self) -> ProviderKind {
        self.provider
    }
}

#[derive(Debug, Serialize)]
pub(super) struct OAuthStartResponse {
    provider: ProviderKind,
    session_id: String,
    authorization_url: String,
    redirect_uri: &'static str,
    expires_in_seconds: u64,
}

impl From<OAuthStartResult> for OAuthStartResponse {
    fn from(result: OAuthStartResult) -> Self {
        Self {
            provider: result.provider(),
            session_id: result.session_id().to_owned(),
            authorization_url: result.authorization_url().to_owned(),
            redirect_uri: result.redirect_uri(),
            expires_in_seconds: result.expires_in_seconds(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthExchangeRequest {
    session_id: String,
    callback_url: String,
}

impl OAuthExchangeRequest {
    pub(super) fn into_parts(self) -> (String, String) {
        (self.session_id, self.callback_url)
    }
}

#[derive(Debug, Serialize)]
pub(super) struct OAuthExchangeResponse {
    provider: ProviderKind,
    account_id: OAuthAccountId,
    label: String,
    max_concurrency: u32,
    enabled: bool,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
    selected_model_count: usize,
    config_version: u64,
    config_revision: u64,
}

impl From<OAuthActivationResult> for OAuthExchangeResponse {
    fn from(result: OAuthActivationResult) -> Self {
        Self {
            provider: result.provider(),
            account_id: result.account_id(),
            label: result.label().to_owned(),
            max_concurrency: result.max_concurrency().get(),
            enabled: result.enabled(),
            safe_account_email: result.safe_account_email().map(str::to_owned),
            expires_at: result.expires_at(),
            selected_model_count: result.selected_model_count(),
            config_version: result.config_version(),
            config_revision: result.config_revision().get(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct OAuthAccountCollectionResponse {
    config_revision: u64,
    items: Vec<OAuthAccountResponse>,
}

impl OAuthAccountCollectionResponse {
    pub(super) fn from_snapshot(snapshot: &PublishedSnapshot) -> Self {
        Self {
            config_revision: snapshot.revision().get(),
            items: snapshot
                .oauth_accounts()
                .accounts()
                .iter()
                .map(OAuthAccountResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthAccountResponse {
    id: OAuthAccountId,
    provider_kind: ProviderKind,
    label: String,
    max_concurrency: u32,
    enabled: bool,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
    token_version: u64,
    account_generation: u64,
    config_version: u64,
    selected_model_count: usize,
    models: Vec<String>,
}

impl From<&OAuthAccount> for OAuthAccountResponse {
    fn from(account: &OAuthAccount) -> Self {
        Self {
            id: account.id(),
            provider_kind: account.provider_kind(),
            label: account.label().to_owned(),
            max_concurrency: account.max_concurrency().get(),
            enabled: account.enabled(),
            safe_account_email: account.safe_account_email().map(str::to_owned),
            expires_at: account.expires_at(),
            token_version: account.token_version(),
            account_generation: account.account_generation(),
            config_version: account.config_version(),
            selected_model_count: account.models().len(),
            models: account
                .models()
                .iter()
                .map(|model| model.as_str().to_owned())
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthAccountUpdateRequest {
    expected_revision: u64,
    expected_config_version: u64,
    label: String,
    max_concurrency: u32,
    enabled: bool,
}

impl OAuthAccountUpdateRequest {
    pub(super) fn into_domain(
        self,
    ) -> Result<(ConfigRevision, u64, OAuthAccountDraft), AdminApiError> {
        let max_concurrency = MaxConcurrency::new(self.max_concurrency)
            .map_err(|error| AdminApiError::invalid_oauth_account(error.to_string()))?;
        let draft = OAuthAccountDraft::new(self.label, max_concurrency, self.enabled)
            .map_err(|error| AdminApiError::invalid_oauth_account(error.to_string()))?;
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(self.expected_config_version)?,
            draft,
        ))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthAccountModelsRequest {
    expected_revision: u64,
    expected_config_version: u64,
    models: Vec<String>,
}

impl OAuthAccountModelsRequest {
    pub(super) fn into_domain(self) -> Result<(ConfigRevision, u64, Vec<String>), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(self.expected_config_version)?,
            self.models,
        ))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OAuthAccountDeleteQuery {
    expected_revision: u64,
    expected_config_version: u64,
}

impl OAuthAccountDeleteQuery {
    pub(super) fn into_domain(self) -> Result<(ConfigRevision, u64), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(self.expected_config_version)?,
        ))
    }
}

fn parse_version(value: u64) -> Result<u64, AdminApiError> {
    (value > 0)
        .then_some(value)
        .ok_or_else(|| AdminApiError::invalid_request("expected_config_version is invalid"))
}
