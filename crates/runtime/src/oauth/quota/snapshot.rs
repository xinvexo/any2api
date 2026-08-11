//! Construction and persistence of a freshly queried OAuth quota snapshot.

use any2api_domain::OAuthAccountId;
use any2api_provider::api::OAuthQuotaUsage;

use super::{
    coordinator::OAuthQuotaService,
    types::{OAuthQuotaError, OAuthQuotaSnapshot},
};
use crate::oauth::document;

pub(super) async fn build(
    service: &OAuthQuotaService,
    id: OAuthAccountId,
    usage: OAuthQuotaUsage,
) -> Result<OAuthQuotaSnapshot, OAuthQuotaError> {
    let fetched_at_ms = document::unix_now_millis();
    let fetched_at = i64::try_from(fetched_at_ms / 1_000).unwrap_or_default();
    let previous = match service.persistence.load(id).await {
        Ok(previous) => previous,
        Err(error) => {
            tracing::warn!(oauth_account_id = %id, error = %error, "previous OAuth quota estimate could not be loaded");
            None
        }
    };
    let published = service.publisher.current_snapshot();
    let account = published
        .oauth_accounts()
        .get(id)
        .ok_or(OAuthQuotaError::AccountNotFound)?;
    let driver = service
        .providers
        .get(account.provider_kind())
        .ok_or(OAuthQuotaError::ProviderUnavailable)?;
    let usd_estimates = match service
        .estimator
        .estimate(
            id,
            &usage,
            previous.as_ref(),
            driver.as_ref(),
            fetched_at_ms,
        )
        .await
    {
        Ok(estimates) => estimates,
        Err(error) => {
            tracing::warn!(oauth_account_id = %id, error = %error, "OAuth quota RequestLog estimate could not be calculated");
            Vec::new()
        }
    };
    let snapshot = OAuthQuotaSnapshot {
        usage,
        usd_estimates,
        fetched_at,
    };
    service.persistence.store(id, &snapshot).await?;
    Ok(snapshot)
}
