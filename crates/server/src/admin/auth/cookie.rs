use axum::http::{HeaderMap, HeaderValue, header::COOKIE};

use super::error::AdminApiError;

pub(super) const ADMIN_COOKIE_NAME: &str = "any2api_admin";

pub(super) fn read(headers: &HeaderMap) -> Result<Option<&str>, AdminApiError> {
    let mut found = None;
    for header in headers.get_all(COOKIE) {
        let value = header
            .to_str()
            .map_err(|_| AdminApiError::session_required())?;
        for part in value.split(';') {
            let Some((name, value)) = part.trim().split_once('=') else {
                continue;
            };
            if name != ADMIN_COOKIE_NAME {
                continue;
            }
            if value.is_empty() || found.is_some_and(|existing| existing != value) {
                return Err(AdminApiError::session_required());
            }
            found = Some(value);
        }
    }
    Ok(found)
}

pub(super) fn issue(
    token: &str,
    secure: bool,
    absolute_timeout_secs: u64,
) -> Result<HeaderValue, AdminApiError> {
    let max_age = absolute_timeout_secs;
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{ADMIN_COOKIE_NAME}={token}; Path=/api/admin; HttpOnly; SameSite=Strict; Max-Age={max_age}{secure}"
    ))
    .map_err(|_| AdminApiError::internal())
}

pub(super) fn clear(secure: bool) -> HeaderValue {
    let secure = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{ADMIN_COOKIE_NAME}=; Path=/api/admin; HttpOnly; SameSite=Strict; Max-Age=0{secure}"
    ))
    .expect("static administrator cookie")
}
