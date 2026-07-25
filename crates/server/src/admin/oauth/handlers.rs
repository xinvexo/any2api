use std::str::FromStr;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        DefaultBodyLimit, Multipart, Path, Query, State,
        multipart::MultipartRejection,
        rejection::{JsonRejection, QueryRejection},
    },
    http::StatusCode,
    response::Response,
    routing::{get, post},
};

use crate::state::AppState;

use super::{
    dto::{
        OAuthAccountCollectionResponse, OAuthAccountDeleteQuery, OAuthAccountModelsRequest,
        OAuthAccountUpdateRequest,
    },
    error::AdminApiError,
    import_dto::OAuthImportResponse,
    import_error,
    login_dto::{
        OAuthDevicePollRequest, OAuthDevicePollResponse, OAuthExchangeRequest,
        OAuthExchangeResponse, OAuthStartRequest, OAuthStartResponse,
    },
    no_store,
    quota_dto::{OAuthQuotaResetResponse, OAuthQuotaResponse},
    quota_error, runtime_error,
};

const MAX_OAUTH_IMPORT_FILES: usize = 32;
const MAX_OAUTH_IMPORT_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_OAUTH_IMPORT_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const MAX_OAUTH_IMPORT_REQUEST_BYTES: usize = 10 * 1024 * 1024;

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/oauth/start", post(start))
        .route("/oauth/exchange", post(exchange))
        .route("/oauth/device/poll", post(poll_device))
        .route(
            "/oauth/import",
            post(import).layer(DefaultBodyLimit::max(MAX_OAUTH_IMPORT_REQUEST_BYTES)),
        )
        .route("/oauth/accounts", get(list))
        .route(
            "/oauth/accounts/{id}",
            axum::routing::patch(update).delete(delete),
        )
        .route(
            "/oauth/accounts/{id}/models",
            axum::routing::put(set_models),
        )
        .route("/oauth/accounts/{id}/quota", get(quota))
        .route("/oauth/accounts/{id}/quota/reset", post(reset_quota))
}

pub(super) async fn import(
    State(state): State<AppState>,
    multipart: Result<Multipart, MultipartRejection>,
) -> Result<Response, AdminApiError> {
    let mut multipart = multipart.map_err(|_| invalid_multipart(StatusCode::BAD_REQUEST))?;
    let mut files = Vec::new();
    let mut total_bytes = 0_usize;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| invalid_multipart(error.status()))?
    {
        if field.name() != Some("files") || field.file_name().is_none() {
            return Err(AdminApiError::new(
                StatusCode::BAD_REQUEST,
                "oauth_import_invalid_multipart",
                "OAuth import accepts only JSON file parts named files",
            ));
        }
        if files.len() >= MAX_OAUTH_IMPORT_FILES {
            return Err(AdminApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "oauth_import_too_many_files",
                "OAuth import accepts at most 32 files",
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|error| invalid_multipart(error.status()))?
        {
            if bytes.len().saturating_add(chunk.len()) > MAX_OAUTH_IMPORT_FILE_BYTES {
                return Err(AdminApiError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "oauth_import_file_too_large",
                    "an OAuth import file exceeds 2 MiB",
                ));
            }
            if total_bytes.saturating_add(chunk.len()) > MAX_OAUTH_IMPORT_TOTAL_BYTES {
                return Err(AdminApiError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "oauth_import_total_too_large",
                    "OAuth import file content exceeds 8 MiB",
                ));
            }
            total_bytes += chunk.len();
            bytes.extend_from_slice(&chunk);
        }
        files.push(Bytes::from(bytes));
    }
    if files.is_empty() {
        return Err(AdminApiError::new(
            StatusCode::BAD_REQUEST,
            "oauth_import_no_files",
            "OAuth import requires at least one JSON file",
        ));
    }
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .import_files(files)
        .await
        .map_err(import_error::map)?;
    Ok(no_store::json(OAuthImportResponse::from(result)))
}

pub(super) async fn list(State(state): State<AppState>) -> Result<Response, AdminApiError> {
    Ok(accounts_response(&state, &state.snapshots().load()).await)
}

pub(super) async fn start(
    State(state): State<AppState>,
    payload: Result<Json<OAuthStartRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let request = parse_json(payload)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .start(request.provider())
        .await
        .map_err(runtime_error::map)?;
    Ok(no_store::json(OAuthStartResponse::from(result)))
}

pub(super) async fn exchange(
    State(state): State<AppState>,
    payload: Result<Json<OAuthExchangeRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let (session_id, callback_url) = parse_json(payload)?.into_parts();
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .exchange(&session_id, &callback_url)
        .await
        .map_err(runtime_error::map)?;
    Ok(no_store::json(OAuthExchangeResponse::from(result)))
}

pub(super) async fn poll_device(
    State(state): State<AppState>,
    payload: Result<Json<OAuthDevicePollRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let session_id = parse_json(payload)?.into_session_id();
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .poll_device(&session_id)
        .await
        .map_err(runtime_error::map)?;
    Ok(no_store::json(OAuthDevicePollResponse::from(result)))
}

pub(super) async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<OAuthAccountUpdateRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_account_id(&id)?;
    let (expected, expected_config_version, draft) = parse_json(payload)?.into_domain()?;
    let snapshot = state
        .publisher()
        .update_oauth_account(expected, id, expected_config_version, draft)
        .await?;
    Ok(accounts_response(&state, &snapshot).await)
}

pub(super) async fn set_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<OAuthAccountModelsRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_account_id(&id)?;
    let (expected, expected_config_version, models) = parse_json(payload)?.into_domain()?;
    let snapshot = state
        .publisher()
        .set_oauth_account_models(expected, id, expected_config_version, models)
        .await?;
    Ok(accounts_response(&state, &snapshot).await)
}

pub(super) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    query: Result<Query<OAuthAccountDeleteQuery>, QueryRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_account_id(&id)?;
    let (expected, expected_config_version) = query
        .map_err(|_| {
            AdminApiError::invalid_request(
                "expected_revision and expected_config_version queries are required",
            )
        })?
        .0
        .into_domain()?;
    let snapshot = state
        .publisher()
        .delete_oauth_account(expected, id, expected_config_version)
        .await?;
    Ok(accounts_response(&state, &snapshot).await)
}

pub(super) async fn quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AdminApiError> {
    let id = parse_account_id(&id)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service.query_quota(id).await.map_err(quota_error::map)?;
    Ok(no_store::json(OAuthQuotaResponse::from(result)))
}

pub(super) async fn reset_quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AdminApiError> {
    let id = parse_account_id(&id)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service.reset_quota(id).await.map_err(quota_error::map)?;
    Ok(no_store::json(OAuthQuotaResetResponse::from(result)))
}

async fn accounts_response(
    state: &AppState,
    snapshot: &any2api_runtime::api::PublishedSnapshot,
) -> Response {
    let usage = super::upstream_usage::load(state).await;
    no_store::json(OAuthAccountCollectionResponse::from_snapshot(
        snapshot, &usage,
    ))
}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, AdminApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))
}

fn oauth_unavailable() -> AdminApiError {
    AdminApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "oauth_unavailable",
        "OAuth2 login is unavailable",
    )
}

fn invalid_multipart(status: StatusCode) -> AdminApiError {
    let status = if status == StatusCode::PAYLOAD_TOO_LARGE {
        StatusCode::PAYLOAD_TOO_LARGE
    } else {
        StatusCode::BAD_REQUEST
    };
    AdminApiError::new(
        status,
        "oauth_import_invalid_multipart",
        "OAuth import must be valid multipart form data",
    )
}

fn parse_account_id(value: &str) -> Result<any2api_domain::OAuthAccountId, AdminApiError> {
    any2api_domain::OAuthAccountId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("OAuth account id is invalid"))
}
