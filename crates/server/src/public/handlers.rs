use any2api_domain::ProtocolOperation;
use any2api_runtime::api::PublicRequest;
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
    execute_json(
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
    execute_json(
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
    execute_json(
        state,
        authenticated,
        headers,
        body,
        ProtocolOperation::Messages,
    )
    .await
}

pub(crate) async fn not_implemented(
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
) -> Result<Response, PublicApiError> {
    let _ = (authenticated.id(), authenticated.snapshot().revision());
    Err(PublicApiError::not_implemented())
}

async fn execute_json(
    state: AppState,
    authenticated: AuthenticatedGatewayApiKey,
    headers: HeaderMap,
    body: Bytes,
    operation: ProtocolOperation,
) -> Response {
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
    let mut outgoing = Response::new(Body::from(response.body));
    *outgoing.status_mut() = response.status;
    *outgoing.headers_mut() = response.headers;
    outgoing
}
