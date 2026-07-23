use std::time::Duration;

use any2api_domain::RetrySafety;
use any2api_provider::api::OAuthRequestPlan;
use any2api_transport::api::{
    BoxByteStream, EndpointNetworkPolicy, TransportError, TransportErrorStage,
    TransportFailureScope, TransportManager, TransportRequest,
};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use tokio::time::timeout;

use crate::published_snapshot::PublishedSnapshot;

use super::error::ProviderOAuthError;

const MAX_TOKEN_RESPONSE_BYTES: usize = 64 * 1024;

pub(super) async fn execute(
    transport: &dyn TransportManager,
    snapshot: &PublishedSnapshot,
    proxy_id: any2api_domain::ProxyProfileId,
    plan: OAuthRequestPlan,
) -> Result<Bytes, ProviderOAuthError> {
    let proxy = snapshot
        .resolved_transport_proxy_for_profile(proxy_id)
        .ok_or(ProviderOAuthError::ProxyNotFound)?;
    if !proxy.profile().enabled() {
        return Err(ProviderOAuthError::ProxyDisabled);
    }
    let request = TransportRequest {
        method: plan.method,
        uri: plan.url.as_str().parse().map_err(|_| {
            ProviderOAuthError::Provider(any2api_provider::ProviderError::InvalidEndpoint(
                "OAuth token URI is invalid".into(),
            ))
        })?,
        headers: plan.headers,
        body: Bytes::from(plan.body),
        network_policy: EndpointNetworkPolicy::new()
            .with_strict_ssrf(snapshot.settings().upstream().strict_ssrf()),
        read_timeout: Duration::from_secs(snapshot.settings().upstream().read_timeout_secs()),
    };
    let response = transport.execute(proxy, request).await?;
    if !response.status.is_success() {
        return Err(ProviderOAuthError::TokenRejected(response.status.as_u16()));
    }
    collect(
        response.body,
        Duration::from_secs(snapshot.settings().upstream().read_timeout_secs()),
        response.read_failure_scope,
    )
    .await
}

async fn collect(
    mut body: BoxByteStream,
    read_timeout: Duration,
    failure_scope: TransportFailureScope,
) -> Result<Bytes, ProviderOAuthError> {
    let mut collected = BytesMut::new();
    loop {
        let next = timeout(read_timeout, body.next()).await.map_err(|_| {
            ProviderOAuthError::Transport(TransportError::new(
                TransportErrorStage::ReadBody,
                failure_scope,
                RetrySafety::Ambiguous,
                "OAuth token response read timed out",
            ))
        })?;
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk?;
        if collected.len().saturating_add(chunk.len()) > MAX_TOKEN_RESPONSE_BYTES {
            return Err(ProviderOAuthError::TokenResponseTooLarge);
        }
        collected.extend_from_slice(&chunk);
    }
    Ok(collected.freeze())
}
