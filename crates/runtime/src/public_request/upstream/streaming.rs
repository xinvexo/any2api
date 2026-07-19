use std::sync::Arc;

use any2api_protocol::api::{DecodedRequest, ProtocolAdapter};
use any2api_provider::api::ProviderRegistry;
use any2api_transport::api::TransportManager;
use http::{HeaderValue, header};

use super::super::{
    PublicResponse, PublicResponseBody,
    affinity::{AffinitySelection, commit_soft_binding},
    response::{collect_body, sanitize_response_headers},
    stream::GuardedBody,
};
use super::{
    failure::AttemptFailure,
    prepared::{AttemptInput, hard_committer, prepare_input},
};
use crate::published_snapshot::PublishedSnapshot;

pub(in crate::public_request) async fn execute_stream_attempt(
    snapshot: &PublishedSnapshot,
    adapter: Arc<dyn ProtocolAdapter>,
    decoded: DecodedRequest,
    public_model: String,
    affinity: AffinitySelection,
    providers: &ProviderRegistry,
    transport: &dyn TransportManager,
) -> Result<PublicResponse, AttemptFailure> {
    let AttemptInput {
        mut prepared,
        candidate,
        target,
        soft_lease,
        fixed,
    } = prepare_input(snapshot, adapter.as_ref(), decoded, affinity, providers)?;
    let response = match prepared.send(transport).await {
        Ok(response) => response,
        Err(error) => {
            prepared.transport_failure(&error);
            return Err(AttemptFailure::transport(error, candidate, fixed));
        }
    };
    let status = response.status;
    let mut headers = response.headers;
    if !status.is_success() {
        let body = match collect_body(response.body).await {
            Ok(body) => body,
            Err(error) => {
                prepared.invalid_response();
                return Err(AttemptFailure::Public(error));
            }
        };
        let classification = prepared.classify(status, &headers, &body);
        prepared.upstream_failure(classification);
        return Err(AttemptFailure::upstream(classification, candidate, fixed));
    }
    sanitize_response_headers(&mut headers);
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    let hard_affinity = hard_committer(snapshot, prepared.operation, target.clone());
    let (permit, health) = prepared.take_guards();
    let body = GuardedBody::new(
        response.body,
        adapter,
        public_model,
        permit,
        health,
        hard_affinity,
    )
    .prime()
    .await
    .map_err(AttemptFailure::Public)?;
    commit_soft_binding(soft_lease, target).map_err(AttemptFailure::Public)?;
    Ok(PublicResponse {
        status,
        headers,
        body: PublicResponseBody::Streaming(body),
    })
}
