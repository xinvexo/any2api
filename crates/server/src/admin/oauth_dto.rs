use any2api_domain::ProviderKind;
use any2api_runtime::api::OAuthStartResult;
use serde::{Deserialize, Serialize};

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
