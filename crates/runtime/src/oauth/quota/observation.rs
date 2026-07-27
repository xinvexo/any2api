use std::time::Duration;

use any2api_provider::api::{
    OAuthQuotaUsage, OAuthRequestPlan, ProviderDriver, UpstreamResponseMeta,
};
use any2api_transport::api::{TransportManager, TransportProxy};

use super::{
    request::{self, OAuthQuotaResponse},
    types::OAuthQuotaError,
};

pub(super) async fn resolve_usage(
    driver: &dyn ProviderDriver,
    transport: &dyn TransportManager,
    proxy: TransportProxy<'_>,
    strict_ssrf: bool,
    read_timeout: Duration,
    response: OAuthQuotaResponse,
    supplement_plan: Option<OAuthRequestPlan>,
) -> Result<OAuthQuotaUsage, OAuthQuotaError> {
    let response_meta = UpstreamResponseMeta {
        status: response.status,
        headers: response.headers.clone(),
    };
    let mut usage = driver
        .parse_oauth_quota_usage(&response_meta, &response.body)
        .map_err(OAuthQuotaError::Provider)?;
    let Some(plan) = supplement_plan else {
        return Ok(usage);
    };
    let response = request::execute(transport, proxy, strict_ssrf, read_timeout, plan).await?;
    if !response.status.is_success() {
        return Err(request::rejection(response.status));
    }
    let supplement = driver
        .parse_oauth_quota_supplement(&response.body)
        .map_err(OAuthQuotaError::Provider)?;
    usage.apply_supplement(supplement);
    Ok(usage)
}
