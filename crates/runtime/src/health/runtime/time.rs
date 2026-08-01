use std::time::{Duration, SystemTime};

use any2api_domain::{MAX_RETRY_AFTER_SECONDS, RetryAfterHint};
use tokio::time::Instant;

const MAX_HEALTH_DELAY: Duration = Duration::from_secs(MAX_RETRY_AFTER_SECONDS);

pub(super) fn retry_delay(hint: Option<RetryAfterHint>, fallback: Duration) -> Duration {
    hint.map(|value| value.delay_from(SystemTime::now()))
        .unwrap_or(fallback)
        .min(MAX_HEALTH_DELAY)
}

pub(super) fn deadline(now: Instant, delay: Duration) -> Instant {
    now.checked_add(delay.min(MAX_HEALTH_DELAY))
        .expect("bounded health deadline must fit in tokio::time::Instant")
}

pub(super) fn max_deadline(current: Option<Instant>, next: Option<Instant>) -> Option<Instant> {
    current.into_iter().chain(next).max()
}
