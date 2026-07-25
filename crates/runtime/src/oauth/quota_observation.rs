use std::time::Duration;

use any2api_provider::api::{
    OAuthQuotaUsage, OAuthQuotaUsageParse, OAuthRequestPlan, ProviderDriver, ProviderError,
    UpstreamResponseMeta,
};
use any2api_transport::api::{TransportManager, TransportProxy};
use http::StatusCode;

use super::{
    quota_request::{self, OAuthQuotaResponse},
    quota_types::OAuthQuotaError,
};

pub(super) async fn resolve_usage(
    driver: &dyn ProviderDriver,
    transport: &dyn TransportManager,
    proxy: TransportProxy<'_>,
    strict_ssrf: bool,
    read_timeout: Duration,
    response: OAuthQuotaResponse,
    probe_plan: Option<OAuthRequestPlan>,
) -> Result<OAuthQuotaUsage, OAuthQuotaError> {
    let parsed = driver
        .parse_oauth_quota_usage(&response_meta(&response), &response.body)
        .map_err(OAuthQuotaError::Provider)?;
    match parsed {
        OAuthQuotaUsageParse::Complete(usage) => Ok(usage),
        OAuthQuotaUsageParse::ProbeRequired => {
            let plan = probe_plan.ok_or_else(|| {
                OAuthQuotaError::Provider(ProviderError::InvalidResponse(
                    "OAuth quota probe plan is missing".into(),
                ))
            })?;
            let response =
                quota_request::execute_headers(transport, proxy, strict_ssrf, read_timeout, plan)
                    .await?;
            if !response.status.is_success() && response.status != StatusCode::TOO_MANY_REQUESTS {
                return Err(OAuthQuotaError::UpstreamRejected(response.status.as_u16()));
            }
            driver
                .parse_oauth_quota_probe(&response_meta(&response))
                .map_err(OAuthQuotaError::Provider)
        }
    }
}

fn response_meta(response: &OAuthQuotaResponse) -> UpstreamResponseMeta {
    UpstreamResponseMeta {
        status: response.status,
        headers: response.headers.clone(),
    }
}
