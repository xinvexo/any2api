//! Construction and persistence of a freshly queried OAuth quota snapshot.

use any2api_domain::OAuthAccountId;

use super::{
    coordinator::{OAuthQuotaService, QueriedQuota},
    persistence::StoredQuotaTelemetry,
    types::{OAuthQuotaError, OAuthQuotaSnapshot},
};
use crate::oauth::document;

pub(super) async fn build(
    service: &OAuthQuotaService,
    id: OAuthAccountId,
    observation: QueriedQuota,
) -> Result<OAuthQuotaSnapshot, OAuthQuotaError> {
    let fetched_at_ms = document::unix_now_millis();
    let fetched_at = i64::try_from(fetched_at_ms / 1_000).unwrap_or_default();
    let checkpoint = service.telemetry.quota_checkpoint().await;
    let previous = match service.persistence.load(id).await {
        Ok(previous) => previous,
        Err(error) => {
            tracing::warn!(oauth_account_id = %id, error = %error, "previous OAuth quota telemetry could not be loaded");
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
    let (estimator_state, estimates) = if let Some(unit) = driver.oauth_quota_cost_unit() {
        let result = service
            .estimator
            .observe(
                id,
                &observation.usage,
                previous.and_then(|value| value.estimator_state),
                observation.credential_fingerprint,
                unit,
                checkpoint,
                fetched_at_ms,
                Some(service.telemetry.as_ref()),
            )
            .await;
        (Some(result.state), result.estimates)
    } else {
        (None, Vec::new())
    };
    let stored = StoredQuotaTelemetry {
        usage: observation.usage.clone(),
        estimator_state,
        fetched_at,
    };
    service.persistence.store(id, &stored).await?;
    Ok(OAuthQuotaSnapshot {
        usage: observation.usage,
        estimates,
        fetched_at,
    })
}
