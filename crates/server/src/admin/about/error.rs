use any2api_updater::api::{UpdateError, UpdateErrorKind};
use axum::http::StatusCode;

use crate::admin::AdminApiError;

pub(super) fn map_error(error: UpdateError) -> AdminApiError {
    let kind = error.kind();
    tracing::warn!(?kind, %error, "application update operation failed");
    let status = match kind {
        UpdateErrorKind::Unsupported | UpdateErrorKind::NoUpdate | UpdateErrorKind::InProgress => {
            StatusCode::CONFLICT
        }
        UpdateErrorKind::CheckFailed
        | UpdateErrorKind::InvalidRelease
        | UpdateErrorKind::DownloadFailed
        | UpdateErrorKind::VerificationFailed => StatusCode::BAD_GATEWAY,
        UpdateErrorKind::InstallFailed => StatusCode::INTERNAL_SERVER_ERROR,
    };
    AdminApiError::new(status, stable_error_code(kind), stable_error_message(kind))
}

pub(super) const fn stable_error_code(kind: UpdateErrorKind) -> &'static str {
    match kind {
        UpdateErrorKind::Unsupported => "update_unsupported",
        UpdateErrorKind::NoUpdate => "update_not_available",
        UpdateErrorKind::InProgress => "update_in_progress",
        UpdateErrorKind::CheckFailed | UpdateErrorKind::InvalidRelease => "update_check_failed",
        UpdateErrorKind::DownloadFailed => "update_download_failed",
        UpdateErrorKind::VerificationFailed => "update_verification_failed",
        UpdateErrorKind::InstallFailed => "update_install_failed",
    }
}

const fn stable_error_message(kind: UpdateErrorKind) -> &'static str {
    match kind {
        UpdateErrorKind::Unsupported => {
            "this runtime environment does not support automatic updates"
        }
        UpdateErrorKind::NoUpdate => "the current version is already up to date",
        UpdateErrorKind::InProgress => "an application update is already in progress",
        UpdateErrorKind::CheckFailed | UpdateErrorKind::InvalidRelease => {
            "the latest official release could not be verified"
        }
        UpdateErrorKind::DownloadFailed => "the official release could not be downloaded",
        UpdateErrorKind::VerificationFailed => "the downloaded release did not pass verification",
        UpdateErrorKind::InstallFailed => {
            "the verified release could not replace the current executable"
        }
    }
}
