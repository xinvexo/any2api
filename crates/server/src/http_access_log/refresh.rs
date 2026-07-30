use axum::http::HeaderMap;

const REFRESH_KIND_HEADER: &str = "x-any2api-log-refresh";
const AUTOMATIC_REFRESH: &[u8] = b"automatic";

pub(crate) fn is_automatic_log_refresh(headers: &HeaderMap) -> bool {
    headers
        .get(REFRESH_KIND_HEADER)
        .is_some_and(|value| value.as_bytes() == AUTOMATIC_REFRESH)
}
