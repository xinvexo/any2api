use any2api_runtime::api::{ConfigPublishError, OAuthImportError, OAuthImportFailureKind};
use axum::http::StatusCode;

use crate::admin::error::AdminApiError;

pub(super) fn map(error: OAuthImportError) -> AdminApiError {
    match error {
        OAuthImportError::NoFiles => AdminApiError::new(
            StatusCode::BAD_REQUEST,
            "oauth_import_no_files",
            "OAuth import requires at least one JSON file",
        ),
        OAuthImportError::TooManyAccounts => AdminApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "oauth_import_too_many_accounts",
            "OAuth import contains too many accounts",
        ),
        OAuthImportError::InvalidFile {
            file_index,
            account_index,
            kind,
        } => invalid_file(file_index, account_index, kind),
        OAuthImportError::Activation(ConfigPublishError::ShuttingDown) => {
            AdminApiError::shutting_down()
        }
        OAuthImportError::Activation(error) => {
            tracing::error!(error = ?error, "OAuth JSON import activation failed");
            AdminApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "oauth_import_activation_failed",
                "OAuth accounts could not be activated",
            )
        }
    }
}

fn invalid_file(
    file_index: usize,
    account_index: Option<usize>,
    kind: OAuthImportFailureKind,
) -> AdminApiError {
    let location = account_index.map_or_else(
        || format!("file {file_index}"),
        |account| format!("file {file_index}, account {account}"),
    );
    let (code, reason) = match kind {
        OAuthImportFailureKind::InvalidJson => ("oauth_import_invalid_json", "is not valid JSON"),
        OAuthImportFailureKind::InvalidEnvelope => (
            "oauth_import_invalid_envelope",
            "has an unsupported account envelope",
        ),
        OAuthImportFailureKind::Empty => ("oauth_import_empty", "does not contain any accounts"),
        OAuthImportFailureKind::TooManyAccounts => (
            "oauth_import_too_many_accounts",
            "contains too many accounts",
        ),
        OAuthImportFailureKind::UnsupportedAccount => (
            "oauth_import_unsupported_account",
            "is not a supported OAuth account",
        ),
        OAuthImportFailureKind::AmbiguousAccount => (
            "oauth_import_ambiguous_account",
            "matches multiple OAuth providers",
        ),
        OAuthImportFailureKind::InvalidAccount => (
            "oauth_import_invalid_account",
            "contains invalid OAuth data",
        ),
    };
    AdminApiError::new(
        StatusCode::BAD_REQUEST,
        code,
        format!("OAuth import {location} {reason}"),
    )
}
