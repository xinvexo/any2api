use std::time::Instant;

use any2api_domain::TokenUsage;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RequestObservation {
    first_token_ms: Option<u64>,
    token_usage: TokenUsage,
}

impl RequestObservation {
    pub(super) fn observe_first_token(&mut self, started_at: Instant) {
        if self.first_token_ms.is_none() {
            self.first_token_ms =
                Some(u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX));
        }
    }

    pub(super) fn observe_token_usage(&mut self, usage: TokenUsage) {
        self.token_usage.merge(usage);
    }

    pub(super) const fn first_token_ms(self) -> Option<u64> {
        self.first_token_ms
    }

    pub(super) const fn token_usage(self) -> TokenUsage {
        self.token_usage
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use any2api_domain::TokenUsage;

    use super::RequestObservation;

    #[test]
    fn usage_updates_are_merged_by_field() {
        let mut observation = RequestObservation::default();
        observation.observe_token_usage(TokenUsage::new(Some(10), Some(1), Some(2), None));
        observation.observe_token_usage(TokenUsage::new(None, Some(7), None, Some(3)));

        assert_eq!(
            observation.token_usage(),
            TokenUsage::new(Some(10), Some(7), Some(2), Some(3))
        );
    }

    #[test]
    fn first_token_is_first_write_wins() {
        let mut observation = RequestObservation::default();
        let started_at = Instant::now() - Duration::from_millis(5);

        observation.observe_first_token(started_at);
        let first = observation.first_token_ms().expect("first token time");
        observation.observe_first_token(Instant::now() - Duration::from_secs(1));

        assert_eq!(observation.first_token_ms(), Some(first));
    }
}
