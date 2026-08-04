use any2api_provider::api::{OAuthQuotaUsage, OAuthRequestPlan, UpstreamResponseMeta};

use super::{rejection::RequestContext, request::OAuthQuotaResponse, types::OAuthQuotaError};

pub(super) async fn resolve_usage(
    context: &RequestContext<'_>,
    response: OAuthQuotaResponse,
    supplement_plan: Option<OAuthRequestPlan>,
) -> Result<OAuthQuotaUsage, OAuthQuotaError> {
    let response_meta = UpstreamResponseMeta {
        status: response.status,
        headers: response.headers.clone(),
    };
    let mut usage = context
        .driver()
        .parse_oauth_quota_usage(&response_meta, &response.body)
        .map_err(OAuthQuotaError::Provider)?;
    let Some(plan) = supplement_plan else {
        return Ok(usage);
    };
    let response = context.execute(plan).await?;
    if !response.status.is_success() {
        return Err(context.rejection(&response));
    }
    let supplement = context
        .driver()
        .parse_oauth_quota_supplement(&response.body)
        .map_err(OAuthQuotaError::Provider)?;
    usage.apply_supplement(supplement);
    Ok(usage)
}
