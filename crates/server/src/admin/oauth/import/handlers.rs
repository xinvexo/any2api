use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Multipart, State, multipart::MultipartRejection},
    http::StatusCode,
    response::Response,
    routing::post,
};

use crate::{
    admin::{error::AdminApiError, no_store},
    state::AppState,
};

use super::{dto::OAuthImportResponse, error};

const MAX_OAUTH_IMPORT_FILES: usize = 32;
const MAX_OAUTH_IMPORT_FILE_BYTES: usize = 2 * 1024 * 1024;
const MAX_OAUTH_IMPORT_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const MAX_OAUTH_IMPORT_REQUEST_BYTES: usize = 10 * 1024 * 1024;

pub(in crate::admin::oauth) fn routes() -> Router<AppState> {
    Router::new().route(
        "/oauth/import",
        post(import).layer(DefaultBodyLimit::max(MAX_OAUTH_IMPORT_REQUEST_BYTES)),
    )
}

async fn import(
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
    let result = service.import_files(files).await.map_err(error::map)?;
    Ok(no_store::json(OAuthImportResponse::from(result)))
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
