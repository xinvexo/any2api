use any2api_provider::api::{OAuthTokenMaterial, encode_oauth_account_document};
use any2api_storage::api::OAuthAccountDocument;

use super::error::OAuthError;

pub(super) fn build_account_document(
    token: &OAuthTokenMaterial,
) -> Result<OAuthAccountDocument, OAuthError> {
    let bytes =
        encode_oauth_account_document(token).map_err(|_| OAuthError::DocumentSerialization)?;
    OAuthAccountDocument::new(token.provider(), bytes.into())
        .map_err(|_| OAuthError::DocumentSerialization)
}

pub(super) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
}

pub(super) fn unix_now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}
