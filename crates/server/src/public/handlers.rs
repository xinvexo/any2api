use any2api_domain::ProtocolOperation;
use any2api_runtime::api::PublicRequest;
use axum::{
    extract::{Extension, State},
    response::Response,
};

use crate::{http_access_log::HttpRequestId, state::AppState};

use super::{auth::AuthenticatedGatewayApiKey, body::PublicBody};

pub(crate) async fn responses(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<HttpRequestId>,
    request: PublicBody,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        request,
        ProtocolOperation::Responses,
    )
    .await
}

pub(crate) async fn responses_compact(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<HttpRequestId>,
    request: PublicBody,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        request,
        ProtocolOperation::ResponsesCompact,
    )
    .await
}

pub(crate) async fn chat_completions(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<HttpRequestId>,
    request: PublicBody,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        request,
        ProtocolOperation::ChatCompletions,
    )
    .await
}

pub(crate) async fn messages(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<HttpRequestId>,
    request: PublicBody,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        request,
        ProtocolOperation::Messages,
    )
    .await
}

pub(crate) async fn messages_count_tokens(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<HttpRequestId>,
    request: PublicBody,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        request,
        ProtocolOperation::MessagesCountTokens,
    )
    .await
}

pub(crate) async fn images_generations(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<HttpRequestId>,
    request: PublicBody,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        request,
        ProtocolOperation::ImagesGenerations,
    )
    .await
}

pub(crate) async fn images_edits(
    State(state): State<AppState>,
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
    Extension(request_id): Extension<HttpRequestId>,
    request: PublicBody,
) -> Response {
    execute_public_request(
        state,
        authenticated,
        request_id,
        request,
        ProtocolOperation::ImagesEdits,
    )
    .await
}

async fn execute_public_request(
    state: AppState,
    authenticated: AuthenticatedGatewayApiKey,
    request_id: HttpRequestId,
    request: PublicBody,
    operation: ProtocolOperation,
) -> Response {
    let PublicBody { headers, body } = request;
    let response = state
        .public_requests()
        .execute(
            authenticated.snapshot_arc(),
            PublicRequest {
                request_id: request_id.get(),
                gateway_api_key_id: authenticated.id(),
                client_ip: authenticated.client_ip(),
                operation,
                headers,
                body,
            },
        )
        .await;
    super::response::from_runtime(response)
}
