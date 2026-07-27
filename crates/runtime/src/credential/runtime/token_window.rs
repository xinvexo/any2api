use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use any2api_domain::{MAX_TOKEN_COUNT, TokenUsage};
use tokio::time::Instant;

const BUCKET_WIDTH: Duration = Duration::from_secs(60);
const RETENTION: Duration = Duration::from_secs(86_400);

#[derive(Debug, Default)]
pub(super) struct CredentialTokenUsageWindow {
    buckets: Mutex<VecDeque<TokenBucket>>,
}

#[derive(Clone, Copy, Debug)]
struct TokenBucket {
    started_at: Instant,
    tokens: u64,
}

impl CredentialTokenUsageWindow {
    pub(super) fn recorder(self: &Arc<Self>) -> CredentialTokenUsageRecorder {
        CredentialTokenUsageRecorder {
            window: Arc::clone(self),
            usage: TokenUsage::default(),
        }
    }

    pub(super) fn snapshot(&self, window_seconds: u64) -> u64 {
        self.snapshot_at(Duration::from_secs(window_seconds), Instant::now())
    }

    fn record_at(&self, tokens: u64, now: Instant) {
        if tokens == 0 {
            return;
        }
        let mut buckets = self.buckets.lock().expect("token usage window poisoned");
        prune(&mut buckets, now);
        if let Some(bucket) = buckets.back_mut()
            && now.saturating_duration_since(bucket.started_at) < BUCKET_WIDTH
        {
            bucket.tokens = safe_add(bucket.tokens, tokens);
            return;
        }
        buckets.push_back(TokenBucket {
            started_at: now,
            tokens: tokens.min(MAX_TOKEN_COUNT),
        });
    }

    fn snapshot_at(&self, window: Duration, now: Instant) -> u64 {
        let mut buckets = self.buckets.lock().expect("token usage window poisoned");
        prune(&mut buckets, now);
        buckets
            .iter()
            .filter(|bucket| now.saturating_duration_since(bucket.started_at) < window)
            .fold(0, |total, bucket| safe_add(total, bucket.tokens))
    }
}

pub(crate) struct CredentialTokenUsageRecorder {
    window: Arc<CredentialTokenUsageWindow>,
    usage: TokenUsage,
}

impl CredentialTokenUsageRecorder {
    pub(crate) fn observe(&mut self, usage: TokenUsage) {
        self.usage.merge(usage);
    }
}

impl Drop for CredentialTokenUsageRecorder {
    fn drop(&mut self) {
        let Some(tokens) = observed_tokens(self.usage) else {
            return;
        };
        self.window.record_at(tokens, Instant::now());
    }
}

fn observed_tokens(usage: TokenUsage) -> Option<u64> {
    if usage.input_tokens().is_none() && usage.output_tokens().is_none() {
        return None;
    }
    Some(safe_add(
        usage.input_tokens().unwrap_or_default(),
        usage.output_tokens().unwrap_or_default(),
    ))
}

fn prune(buckets: &mut VecDeque<TokenBucket>, now: Instant) {
    while buckets
        .front()
        .is_some_and(|bucket| now.saturating_duration_since(bucket.started_at) >= RETENTION)
    {
        buckets.pop_front();
    }
}

fn safe_add(left: u64, right: u64) -> u64 {
    left.saturating_add(right).min(MAX_TOKEN_COUNT)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn recorder_merges_cumulative_usage_without_double_counting_cache_details() {
        let window = Arc::new(CredentialTokenUsageWindow::default());
        let now = Instant::now();
        let mut recorder = window.recorder();
        recorder.observe(TokenUsage::new(
            Some(400_000),
            Some(10_000),
            Some(300_000),
            None,
        ));
        recorder.observe(TokenUsage::new(None, Some(100_000), None, Some(20_000)));
        let usage = recorder.usage;
        std::mem::forget(recorder);
        window.record_at(observed_tokens(usage).expect("usage"), now);

        assert_eq!(window.snapshot_at(RETENTION, now), 500_000);
    }

    #[test]
    fn rolling_window_expires_usage_after_twenty_four_hours() {
        let window = CredentialTokenUsageWindow::default();
        let now = Instant::now();
        window.record_at(600_000, now);
        window.record_at(250_000, now + BUCKET_WIDTH);

        assert_eq!(window.snapshot_at(RETENTION, now + BUCKET_WIDTH), 850_000);
        assert_eq!(window.snapshot_at(RETENTION, now + RETENTION), 250_000);
        assert_eq!(
            window.snapshot_at(RETENTION, now + RETENTION + BUCKET_WIDTH),
            0
        );
    }
}
