use any2api_domain::ProtocolOperation;
use any2api_runtime::api::{PublicRequest, PublicResponseBody};
use axum::{
    body::{Body, Bytes},
    extract::{Extension, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

use super::{auth::AuthenticatedGatewayApiKey, error::PublicApiError};

pub(crate) async fn responses(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        headers,
        body,
        ProtocolOperation::Responses,
    )
    .await
}

pub(crate) async fn responses_compact(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        headers,
        body,
        ProtocolOperation::ResponsesCompact,
    )
    .await
}

pub(crate) async fn messages(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        headers,
        body,
        ProtocolOperation::Messages,
    )
    .await
}

pub(crate) async fn messages_count_tokens(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        headers,
        body,
        ProtocolOperation::MessagesCountTokens,
    )
    .await
}

async fn execute_public_request(
    state: AppState,
    authenticated: AuthenticatedGatewayApiKey,
    headers: HeaderMap,
    body: Bytes,
    operation: ProtocolOperation,
) -> Response {
    let _gateway_api_key_id = authenticated.id();
    let Some(service) = state.public_requests() else {
        return PublicApiError::not_implemented().into_response();
    };
    let response = service
        .execute(
            authenticated.snapshot_arc(),
            PublicRequest {
                operation,
                headers,
                body,
            },
        )
        .await;
    let body = match response.body {
        PublicResponseBody::Buffered(body) => Body::from(body),
        PublicResponseBody::Streaming(body) => Body::from_stream(body),
    };
    let mut outgoing = Response::new(body);
    *outgoing.status_mut() = response.status;
    *outgoing.headers_mut() = response.headers;
    outgoing
}
