use std::time::Duration;

use any2api_provider::api::OAuthRequestPlan;
use any2api_transport::api::{
    EndpointNetworkPolicy, TransportManager, TransportProxy, TransportRequest,
};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use http::StatusCode;
use tokio::time::timeout;

use super::quota_types::OAuthQuotaError;

const MAX_QUOTA_RESPONSE_BYTES: usize = 128 * 1024;

pub(super) struct OAuthQuotaResponse {
    pub status: StatusCode,
    pub body: Bytes,
}

pub(super) async fn execute(
    transport: &dyn TransportManager,
    proxy: TransportProxy<'_>,
    strict_ssrf: bool,
    read_timeout: Duration,
    plan: OAuthRequestPlan,
) -> Result<OAuthQuotaResponse, OAuthQuotaError> {
    let request = TransportRequest {
        method: plan.method,
        uri: plan
            .url
            .as_str()
            .parse()
            .map_err(|_| OAuthQuotaError::InvalidEndpointUri)?,
        headers: plan.headers,
        body: Bytes::from(plan.body),
        network_policy: EndpointNetworkPolicy::new().with_strict_ssrf(strict_ssrf),
        read_timeout,
    };
    let response = transport
        .execute(proxy, request)
        .await
        .map_err(OAuthQuotaError::Transport)?;
    let body = collect(response.body, read_timeout).await?;
    Ok(OAuthQuotaResponse {
        status: response.status,
        body,
    })
}

async fn collect(
    mut body: any2api_transport::api::BoxByteStream,
    read_timeout: Duration,
) -> Result<Bytes, OAuthQuotaError> {
    let mut collected = BytesMut::new();
    loop {
        let next = timeout(read_timeout, body.next())
            .await
            .map_err(|_| OAuthQuotaError::ReadTimeout)?;
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(OAuthQuotaError::Transport)?;
        if collected.len().saturating_add(chunk.len()) > MAX_QUOTA_RESPONSE_BYTES {
            return Err(OAuthQuotaError::ResponseTooLarge);
        }
        collected.extend_from_slice(&chunk);
    }
    Ok(collected.freeze())
}

#[cfg(test)]
mod tests {
    use futures_util::stream;

    use super::*;

    #[tokio::test]
    async fn quota_response_body_is_bounded() {
        let body: any2api_transport::api::BoxByteStream =
            Box::pin(stream::iter([Ok(Bytes::from(vec![
                0_u8;
                MAX_QUOTA_RESPONSE_BYTES
                    + 1
            ]))]));

        assert!(matches!(
            collect(body, Duration::from_secs(1)).await,
            Err(OAuthQuotaError::ResponseTooLarge)
        ));
    }
}
