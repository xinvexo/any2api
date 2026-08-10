use std::time::Duration;

use any2api_domain::ProviderKind;
use any2api_provider::api::{OAuthRequestPlan, ProviderError};
use any2api_transport::api::{
    EndpointNetworkPolicy, TransportIsolationKey, TransportManager, TransportProxy,
    TransportRequest,
};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use tokio::time::timeout;

use crate::oauth::{control_plane::OAuthControlPlanePacer, error::OAuthError};

const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1024;
const TOKEN_READ_TIMEOUT: Duration = Duration::from_secs(30);

pub(in crate::oauth) struct OAuthHttpResponse {
    pub(in crate::oauth) status: http::StatusCode,
    pub(in crate::oauth) body: Bytes,
}

pub(in crate::oauth) async fn execute(
    transport: &dyn TransportManager,
    control_plane: &OAuthControlPlanePacer,
    provider: ProviderKind,
    proxy: TransportProxy<'_>,
    strict_ssrf: bool,
    isolation: TransportIsolationKey,
    plan: OAuthRequestPlan,
) -> Result<Bytes, OAuthError> {
    let response = execute_response(
        transport,
        control_plane,
        provider,
        proxy,
        strict_ssrf,
        isolation,
        plan,
    )
    .await?;
    if !response.status.is_success() {
        return Err(OAuthError::TokenRejected(response.status.as_u16()));
    }
    Ok(response.body)
}

pub(in crate::oauth) async fn execute_response(
    transport: &dyn TransportManager,
    control_plane: &OAuthControlPlanePacer,
    provider: ProviderKind,
    proxy: TransportProxy<'_>,
    strict_ssrf: bool,
    isolation: TransportIsolationKey,
    plan: OAuthRequestPlan,
) -> Result<OAuthHttpResponse, OAuthError> {
    let request = TransportRequest {
        method: plan.method,
        uri: plan.url.as_str().parse().map_err(|_| {
            OAuthError::Provider(ProviderError::InvalidEndpoint(
                "OAuth token URI is invalid".into(),
            ))
        })?,
        headers: plan.headers,
        body: Bytes::from(plan.body),
        isolation,
        network_policy: EndpointNetworkPolicy::new().with_strict_ssrf(strict_ssrf),
        read_timeout: TOKEN_READ_TIMEOUT,
    };
    control_plane.wait(provider).await;
    let response = transport.execute(proxy, request).await?;
    Ok(OAuthHttpResponse {
        status: response.status,
        body: collect(response.body).await?,
    })
}

async fn collect(mut body: any2api_transport::api::BoxByteStream) -> Result<Bytes, OAuthError> {
    let mut collected = BytesMut::new();
    loop {
        let next = timeout(TOKEN_READ_TIMEOUT, body.next())
            .await
            .map_err(|_| OAuthError::TokenReadTimeout)?;
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk?;
        if collected.len().saturating_add(chunk.len()) > MAX_TOKEN_RESPONSE_BYTES {
            return Err(OAuthError::TokenResponseTooLarge);
        }
        collected.extend_from_slice(&chunk);
    }
    Ok(collected.freeze())
}
