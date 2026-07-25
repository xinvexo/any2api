use std::time::{Duration, SystemTime};

use any2api_domain::{MAX_RETRY_AFTER_SECONDS, RetryAfterHint};
use http::{HeaderMap, header};
use time::OffsetDateTime;

pub(crate) fn retry_after_hint(headers: &HeaderMap) -> Option<RetryAfterHint> {
    let value = headers.get(header::RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(RetryAfterHint::Delay(Duration::from_secs(
            seconds.min(MAX_RETRY_AFTER_SECONDS),
        )));
    }
    let date = OffsetDateTime::parse(value, &time::format_description::well_known::Rfc2822).ok()?;
    let seconds = date.unix_timestamp();
    if seconds < 0 {
        return None;
    }
    SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(seconds as u64))
        .map(RetryAfterHint::At)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use any2api_domain::RetryAfterHint;
    use http::{HeaderMap, HeaderValue, header};

    use super::retry_after_hint;

    #[test]
    fn parses_delta_seconds_and_http_date() {
        let mut headers = HeaderMap::new();
        headers.insert(header::RETRY_AFTER, HeaderValue::from_static("42"));
        assert_eq!(
            retry_after_hint(&headers),
            Some(RetryAfterHint::Delay(Duration::from_secs(42)))
        );

        headers.insert(
            header::RETRY_AFTER,
            HeaderValue::from_static("Sun, 06 Nov 1994 08:49:37 GMT"),
        );
        assert_eq!(
            retry_after_hint(&headers),
            Some(RetryAfterHint::At(
                SystemTime::UNIX_EPOCH + Duration::from_secs(784_111_777)
            ))
        );
    }

    #[test]
    fn clamps_unbounded_delta_seconds() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::RETRY_AFTER,
            HeaderValue::from_static("18446744073709551615"),
        );

        assert_eq!(
            retry_after_hint(&headers),
            Some(RetryAfterHint::Delay(Duration::from_secs(
                30 * 24 * 60 * 60,
            )))
        );
    }
}
