use std::time::Duration;

use any2api_provider::api::OAuthRequestPlan;
use any2api_transport::api::{
    EndpointNetworkPolicy, TransportIsolationKey, TransportManager, TransportProxy,
    TransportRequest, TransportResponse,
};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use http::{HeaderMap, StatusCode};
use tokio::time::timeout;

use super::types::OAuthQuotaError;
use crate::credential::{ModelCatalogReadError, collect_model_catalog};

const MAX_QUOTA_RESPONSE_BYTES: usize = 128 * 1024;

pub(super) struct OAuthQuotaResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

pub(super) async fn execute(
    transport: &dyn TransportManager,
    proxy: TransportProxy<'_>,
    strict_ssrf: bool,
    read_timeout: Duration,
    isolation: TransportIsolationKey,
    plan: OAuthRequestPlan,
) -> Result<OAuthQuotaResponse, OAuthQuotaError> {
    let TransportResponse {
        status,
        headers,
        body,
        ..
    } = start(transport, proxy, strict_ssrf, read_timeout, isolation, plan).await?;
    let body = collect(body, read_timeout).await?;
    Ok(OAuthQuotaResponse {
        status,
        headers,
        body,
    })
}

pub(super) async fn execute_model_catalog(
    transport: &dyn TransportManager,
    proxy: TransportProxy<'_>,
    strict_ssrf: bool,
    read_timeout: Duration,
    isolation: TransportIsolationKey,
    plan: OAuthRequestPlan,
) -> Result<OAuthQuotaResponse, OAuthQuotaError> {
    let TransportResponse {
        status,
        headers,
        body,
        read_failure_scope,
    } = start(transport, proxy, strict_ssrf, read_timeout, isolation, plan).await?;
    let body = collect_model_catalog(body, read_timeout, read_failure_scope)
        .await
        .map_err(|error| match error {
            ModelCatalogReadError::Transport(error) => OAuthQuotaError::Transport(error),
            ModelCatalogReadError::TooLarge => OAuthQuotaError::ModelCatalogResponseTooLarge,
        })?;
    Ok(OAuthQuotaResponse {
        status,
        headers,
        body,
    })
}

async fn start(
    transport: &dyn TransportManager,
    proxy: TransportProxy<'_>,
    strict_ssrf: bool,
    read_timeout: Duration,
    isolation: TransportIsolationKey,
    plan: OAuthRequestPlan,
) -> Result<TransportResponse, OAuthQuotaError> {
    let request = TransportRequest {
        method: plan.method,
        uri: plan
            .url
            .as_str()
            .parse()
            .map_err(|_| OAuthQuotaError::InvalidEndpointUri)?,
        headers: plan.headers,
        body: Bytes::from(plan.body),
        isolation,
        network_policy: EndpointNetworkPolicy::new().with_strict_ssrf(strict_ssrf),
        read_timeout,
    };
    transport
        .execute(proxy, request)
        .await
        .map_err(OAuthQuotaError::Transport)
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

    #[tokio::test]
    async fn model_catalog_body_can_exceed_quota_limit() {
        let body: any2api_transport::api::BoxByteStream =
            Box::pin(stream::iter([Ok(Bytes::from(vec![
                0_u8;
                MAX_QUOTA_RESPONSE_BYTES
                    + 1
            ]))]));

        let collected = collect_model_catalog(
            body,
            Duration::from_secs(1),
            any2api_transport::api::TransportFailureScope::Endpoint,
        )
        .await
        .expect("model catalog body");

        assert_eq!(collected.len(), MAX_QUOTA_RESPONSE_BYTES + 1);
    }
}
