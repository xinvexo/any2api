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

pub(crate) fn retry_after_hint_with_millis(headers: &HeaderMap) -> Option<RetryAfterHint> {
    retry_after_millis_hint(headers).or_else(|| retry_after_hint(headers))
}

fn retry_after_millis_hint(headers: &HeaderMap) -> Option<RetryAfterHint> {
    let value = headers.get("retry-after-ms")?.to_str().ok()?.trim();
    let milliseconds = value.parse::<f64>().ok()?;
    if !milliseconds.is_finite() || milliseconds < 0.0 {
        return None;
    }
    let seconds = (milliseconds / 1_000.0).min(MAX_RETRY_AFTER_SECONDS as f64);
    Duration::try_from_secs_f64(seconds)
        .ok()
        .map(RetryAfterHint::Delay)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use any2api_domain::RetryAfterHint;
    use http::{HeaderMap, HeaderValue, header};

    use super::{retry_after_hint, retry_after_hint_with_millis};

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

    #[test]
    fn millisecond_hint_is_precise_preferred_bounded_and_can_fall_back() {
        let mut headers = HeaderMap::new();
        headers.insert(header::RETRY_AFTER, HeaderValue::from_static("9"));
        headers.insert("retry-after-ms", HeaderValue::from_static("1250"));
        assert_eq!(
            retry_after_hint_with_millis(&headers),
            Some(RetryAfterHint::Delay(Duration::from_millis(1_250)))
        );
        assert_eq!(
            retry_after_hint(&headers),
            Some(RetryAfterHint::Delay(Duration::from_secs(9)))
        );

        headers.insert("retry-after-ms", HeaderValue::from_static("NaN"));
        assert_eq!(
            retry_after_hint_with_millis(&headers),
            Some(RetryAfterHint::Delay(Duration::from_secs(9)))
        );
        headers.insert("retry-after-ms", HeaderValue::from_static("-1"));
        assert_eq!(
            retry_after_hint_with_millis(&headers),
            Some(RetryAfterHint::Delay(Duration::from_secs(9)))
        );

        headers.insert(
            "retry-after-ms",
            HeaderValue::from_static("99999999999999999999"),
        );
        assert_eq!(
            retry_after_hint_with_millis(&headers),
            Some(RetryAfterHint::Delay(Duration::from_secs(
                30 * 24 * 60 * 60,
            )))
        );
    }
}
