use any2api_domain::OAuthAccountId;
use any2api_runtime::api::OAuthQuotaRefreshBatchResult;
use serde::{Deserialize, Serialize};

use crate::admin::error::AdminApiError;

const MAX_BATCH_ACCOUNT_IDS: usize = 1_024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::admin::oauth::quota) struct OAuthQuotaBatchRefreshRequest {
    account_ids: Vec<String>,
}

impl OAuthQuotaBatchRefreshRequest {
    pub(in crate::admin::oauth::quota) fn into_account_ids(
        self,
    ) -> Result<Vec<OAuthAccountId>, AdminApiError> {
        if self.account_ids.len() > MAX_BATCH_ACCOUNT_IDS {
            return Err(AdminApiError::invalid_request(
                "too many OAuth accounts in one quota refresh",
            ));
        }
        self.account_ids
            .into_iter()
            .map(|id| {
                id.parse()
                    .map_err(|_| AdminApiError::invalid_request("OAuth account id is invalid"))
            })
            .collect()
    }
}

#[derive(Debug, Serialize)]
pub(in crate::admin::oauth::quota) struct OAuthQuotaBatchRefreshResponse {
    succeeded_account_ids: Vec<String>,
    failed_account_ids: Vec<String>,
    model_catalog_refreshed_scopes: usize,
    model_catalog_failed_scopes: usize,
}

impl From<OAuthQuotaRefreshBatchResult> for OAuthQuotaBatchRefreshResponse {
    fn from(result: OAuthQuotaRefreshBatchResult) -> Self {
        Self {
            succeeded_account_ids: result.succeeded().iter().map(ToString::to_string).collect(),
            failed_account_ids: result.failed().iter().map(ToString::to_string).collect(),
            model_catalog_refreshed_scopes: result.model_catalog_refreshed_scopes(),
            model_catalog_failed_scopes: result.model_catalog_failed_scopes(),
        }
    }
}
