use std::time::Duration;

use any2api_domain::ProxyProfile;
use any2api_provider::api::OAuthRequestPlan;
use any2api_transport::api::{
    EndpointNetworkPolicy, TransportManager, TransportProxy, TransportRequest,
};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use tokio::time::timeout;

use super::error::OAuthError;

const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1024;
const TOKEN_READ_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) async fn execute(
    transport: &dyn TransportManager,
    plan: OAuthRequestPlan,
) -> Result<Bytes, OAuthError> {
    let direct = ProxyProfile::direct();
    let request = TransportRequest {
        method: plan.method,
        uri: plan.url.as_str().parse().map_err(|_| {
            OAuthError::Provider(any2api_provider::ProviderError::InvalidEndpoint(
                "OAuth token URI is invalid".into(),
            ))
        })?,
        headers: plan.headers,
        body: Bytes::from(plan.body),
        network_policy: EndpointNetworkPolicy::new(),
        read_timeout: TOKEN_READ_TIMEOUT,
    };
    let response = transport
        .execute(TransportProxy::new(&direct, None), request)
        .await?;
    if !response.status.is_success() {
        return Err(OAuthError::TokenRejected(response.status.as_u16()));
    }
    collect(response.body).await
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
