use std::time::Duration;

use any2api_domain::StreamSettings;

#[derive(Debug)]
pub(in crate::public_request) struct PrecommitBudget {
    max_bytes: usize,
    max_duration: Duration,
    used_bytes: usize,
    committed: bool,
}

impl PrecommitBudget {
    pub(in crate::public_request) fn from_settings(settings: &StreamSettings) -> Self {
        Self::new(
            usize::try_from(settings.precommit_max_bytes())
                .expect("validated precommit byte budget fits usize"),
            Duration::from_millis(settings.precommit_max_duration_ms()),
        )
    }

    pub(in crate::public_request) fn new(max_bytes: usize, max_duration: Duration) -> Self {
        debug_assert!(max_bytes > 0);
        debug_assert!(!max_duration.is_zero());
        Self {
            max_bytes,
            max_duration,
            used_bytes: 0,
            committed: false,
        }
    }

    pub(super) const fn max_frame_bytes(&self) -> usize {
        self.max_bytes
    }

    pub(super) const fn max_duration(&self) -> Duration {
        self.max_duration
    }

    pub(super) const fn is_committed(&self) -> bool {
        self.committed
    }

    pub(super) fn observe_frame(&mut self, bytes: usize) -> Result<(), PrecommitBudgetExceeded> {
        if self.committed {
            return Ok(());
        }
        let used_bytes = self
            .used_bytes
            .checked_add(bytes)
            .ok_or(PrecommitBudgetExceeded)?;
        if used_bytes > self.max_bytes {
            return Err(PrecommitBudgetExceeded);
        }
        self.used_bytes = used_bytes;
        Ok(())
    }

    pub(super) fn commit(&mut self) {
        self.committed = true;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PrecommitBudgetExceeded;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::PrecommitBudget;

    #[test]
    fn byte_limit_applies_until_commit() {
        let mut budget = PrecommitBudget::new(8, Duration::from_secs(1));
        assert!(budget.observe_frame(8).is_ok());
        assert!(budget.observe_frame(1).is_err());
    }

    #[test]
    fn committed_stream_is_no_longer_charged_to_precommit_budget() {
        let mut budget = PrecommitBudget::new(1, Duration::from_secs(1));
        budget.observe_frame(1).expect("first event");
        budget.commit();

        assert!(budget.observe_frame(usize::MAX).is_ok());
    }
}
