use any2api_provider::api::{
    OAuthQuotaTokenBalance, OAuthQuotaUsage, OAuthRequestPlan, OAuthTokenMaterial,
    UpstreamResponseMeta,
};

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
        return Err(context.rejection(&response).await);
    }
    let supplement = context
        .driver()
        .parse_oauth_quota_supplement(&response.body)
        .map_err(OAuthQuotaError::Provider)?;
    usage.apply_supplement(supplement);
    Ok(usage)
}

pub(super) async fn resolve_token_balance(
    context: &RequestContext<'_>,
    token: &OAuthTokenMaterial,
    usage: &OAuthQuotaUsage,
) -> Result<Option<OAuthQuotaTokenBalance>, OAuthQuotaError> {
    let Some(plan) = context
        .driver()
        .oauth_quota_token_balance_plan(token, usage)
        .map_err(OAuthQuotaError::Provider)?
    else {
        return Ok(None);
    };
    let response = context.execute(plan).await?;
    let meta = UpstreamResponseMeta {
        status: response.status,
        headers: response.headers,
    };
    let balance = context
        .driver()
        .parse_oauth_quota_token_balance(usage, &meta, &response.body)
        .map_err(OAuthQuotaError::Provider)?;
    if meta.status.is_success() || balance.is_some() {
        Ok(balance)
    } else {
        Err(context
            .rejection(&OAuthQuotaResponse {
                status: meta.status,
                headers: meta.headers,
                body: response.body,
            })
            .await)
    }
}
