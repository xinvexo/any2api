//! Construction and persistence of a freshly queried OAuth quota snapshot.

use any2api_domain::OAuthAccountId;

use super::{
    coordinator::{OAuthQuotaService, QueriedQuota},
    persistence::StoredQuotaTelemetry,
    types::{OAuthQuotaError, OAuthQuotaSnapshot},
};
pub(super) async fn build(
    service: &OAuthQuotaService,
    id: OAuthAccountId,
    observation: QueriedQuota,
) -> Result<OAuthQuotaSnapshot, OAuthQuotaError> {
    let fetched_at_ms = observation.telemetry_observation.observed_at_ms;
    let fetched_at = i64::try_from(fetched_at_ms / 1_000).unwrap_or_default();
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
                observation.telemetry_observation.clone(),
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
