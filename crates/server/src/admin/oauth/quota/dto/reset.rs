use any2api_runtime::api::OAuthQuotaResetOutcome;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::admin::oauth::quota) struct OAuthQuotaResetRequest {
    redeem_request_id: Uuid,
}

impl OAuthQuotaResetRequest {
    pub(in crate::admin::oauth::quota) const fn into_redeem_request_id(self) -> Uuid {
        self.redeem_request_id
    }
}

#[derive(Debug, Serialize)]
pub(in crate::admin::oauth::quota) struct OAuthQuotaResetResponse {
    windows_reset: u32,
}

impl From<OAuthQuotaResetOutcome> for OAuthQuotaResetResponse {
    fn from(value: OAuthQuotaResetOutcome) -> Self {
        Self {
            windows_reset: value.windows_reset,
        }
    }
}
