use any2api_provider::api::{OAuthTokenMaterial, serialize_file};
use any2api_storage::api::OAuthAccountDocument;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::error::OAuthError;

pub(super) fn serialize(token: &OAuthTokenMaterial) -> Result<OAuthAccountDocument, OAuthError> {
    let now = unix_now();
    let last_refresh = format_timestamp(now)?;
    let expired = token
        .expires_at()
        .map(format_timestamp)
        .transpose()?
        .unwrap_or_default();
    let bytes = serialize_file(token, &last_refresh, &expired)
        .map_err(|_| OAuthError::DocumentSerialization)?;
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

fn format_timestamp(timestamp: i64) -> Result<String, OAuthError> {
    let value = OffsetDateTime::from_unix_timestamp(timestamp)
        .map_err(|_| OAuthError::DocumentSerialization)?;
    value
        .format(&Rfc3339)
        .map_err(|_| OAuthError::DocumentSerialization)
}
