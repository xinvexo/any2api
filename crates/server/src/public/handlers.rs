use any2api_domain::ProtocolOperation;
use any2api_runtime::api::PublicRequest;
use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::HeaderMap,
    response::Response,
};

use crate::state::AppState;

use super::{auth::AuthenticatedGatewayApiKey, request_id::PublicRequestId};

pub(crate) async fn responses(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<PublicRequestId>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        headers,
        body,
        ProtocolOperation::Responses,
    )
    .await
}

pub(crate) async fn responses_compact(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<PublicRequestId>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        headers,
        body,
        ProtocolOperation::ResponsesCompact,
    )
    .await
}

pub(crate) async fn messages(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<PublicRequestId>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        headers,
        body,
        ProtocolOperation::Messages,
    )
    .await
}

pub(crate) async fn messages_count_tokens(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<PublicRequestId>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        headers,
        body,
        ProtocolOperation::MessagesCountTokens,
    )
    .await
}

async fn execute_public_request(
    state: AppState,
    authenticated: AuthenticatedGatewayApiKey,
    request_id: PublicRequestId,
    headers: HeaderMap,
    body: Bytes,
    operation: ProtocolOperation,
) -> Response {
    let response = state
        .public_requests()
        .execute(
            authenticated.snapshot_arc(),
            PublicRequest {
                request_id: request_id.get(),
                gateway_api_key_id: authenticated.id(),
                operation,
                headers,
                body,
            },
        )
        .await;
    super::response::from_runtime(response)
}
