//! OAuth import response DTOs without token material.

use any2api_domain::{OAuthAccount, RequestsPerMinute};
use any2api_runtime::api::OAuthImportResult;
use serde::Serialize;

use super::super::proxy_selection::OAuthProxySelectionDto;

#[derive(Serialize)]
pub(super) struct OAuthImportResponse {
    imported_count: usize,
    config_revision: u64,
    items: Vec<OAuthImportedAccountResponse>,
}

impl From<OAuthImportResult> for OAuthImportResponse {
    fn from(result: OAuthImportResult) -> Self {
        let items = result
            .accounts()
            .iter()
            .map(OAuthImportedAccountResponse::from)
            .collect::<Vec<_>>();
        Self {
            imported_count: items.len(),
            config_revision: result.revision().get(),
            items,
        }
    }
}

#[derive(Serialize)]
struct OAuthImportedAccountResponse {
    id: any2api_domain::OAuthAccountId,
    provider_kind: any2api_domain::ProviderKind,
    label: String,
    proxy_selection: OAuthProxySelectionDto,
    requests_per_minute: Option<u32>,
    enabled: bool,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
    selected_model_count: usize,
    config_version: u64,
}

impl From<&OAuthAccount> for OAuthImportedAccountResponse {
    fn from(account: &OAuthAccount) -> Self {
        Self {
            id: account.id(),
            provider_kind: account.provider_kind(),
            label: account.label().to_owned(),
            proxy_selection: account.proxy_selection().into(),
            requests_per_minute: account.requests_per_minute().map(RequestsPerMinute::get),
            enabled: account.enabled(),
            safe_account_email: account.safe_account_email().map(str::to_owned),
            expires_at: account.expires_at(),
            selected_model_count: account.models().len(),
            config_version: account.config_version(),
        }
    }
}
