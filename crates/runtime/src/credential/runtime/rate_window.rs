use std::{collections::VecDeque, time::Duration};

use any2api_domain::RequestsPerMinute;
use tokio::time::Instant;

const RATE_WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CredentialRateSnapshot {
    requests_per_minute: Option<u32>,
    requests_in_window: u32,
    retry_at: Option<Instant>,
}

impl CredentialRateSnapshot {
    #[must_use]
    pub const fn requests_per_minute(self) -> Option<u32> {
        self.requests_per_minute
    }

    #[must_use]
    pub const fn requests_in_window(self) -> u32 {
        self.requests_in_window
    }

    #[must_use]
    pub const fn remaining(self) -> Option<u32> {
        match self.requests_per_minute {
            Some(limit) => Some(limit.saturating_sub(self.requests_in_window)),
            None => None,
        }
    }

    #[must_use]
    pub const fn retry_at(self) -> Option<Instant> {
        self.retry_at
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RateLimited {
    pub(crate) retry_at: Option<Instant>,
}

#[derive(Debug)]
pub(super) struct CredentialRateWindow {
    attempts: VecDeque<Instant>,
}

impl CredentialRateWindow {
    pub(super) const fn new() -> Self {
        Self {
            attempts: VecDeque::new(),
        }
    }

    pub(super) fn try_reserve(
        &mut self,
        now: Instant,
        limit: Option<RequestsPerMinute>,
        fixed_waiters: u32,
        fixed: bool,
    ) -> Result<(), RateLimited> {
        self.prune(now);
        let Some(limit) = limit else {
            return Ok(());
        };
        if !fixed && fixed_waiters > 0 {
            return Err(RateLimited { retry_at: None });
        }
        if self.attempts.len() >= limit.get() as usize {
            return Err(RateLimited {
                retry_at: self.next_available_at(limit),
            });
        }
        self.attempts.push_back(now);
        Ok(())
    }

    pub(super) fn snapshot(
        &mut self,
        now: Instant,
        limit: Option<RequestsPerMinute>,
    ) -> CredentialRateSnapshot {
        self.prune(now);
        let requests_per_minute = limit.map(RequestsPerMinute::get);
        let retry_at = limit.and_then(|limit| self.next_available_at(limit));
        CredentialRateSnapshot {
            requests_per_minute,
            requests_in_window: if limit.is_some() {
                u32::try_from(self.attempts.len())
                    .expect("RPM window is bounded by the validated u32 limit")
            } else {
                0
            },
            retry_at,
        }
    }

    fn prune(&mut self, now: Instant) {
        let cutoff = now.checked_sub(RATE_WINDOW);
        while self
            .attempts
            .front()
            .is_some_and(|started_at| cutoff.is_some_and(|cutoff| *started_at <= cutoff))
        {
            self.attempts.pop_front();
        }
    }

    fn next_available_at(&self, limit: RequestsPerMinute) -> Option<Instant> {
        let limit = limit.get() as usize;
        (self.attempts.len() >= limit)
            .then(|| self.attempts[self.attempts.len() - limit] + RATE_WINDOW)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use any2api_domain::RequestsPerMinute;
    use tokio::time::Instant;

    use super::CredentialRateWindow;

    #[test]
    fn lowering_a_limit_reports_when_enough_old_attempts_expire() {
        let start = Instant::now();
        let mut window = CredentialRateWindow::new();
        window
            .try_reserve(start, Some(rpm(3)), 0, false)
            .expect("first");
        window
            .try_reserve(start + Duration::from_secs(1), Some(rpm(3)), 0, false)
            .expect("second");
        window
            .try_reserve(start + Duration::from_secs(2), Some(rpm(3)), 0, false)
            .expect("third");

        let limited = window
            .try_reserve(start + Duration::from_secs(3), Some(rpm(1)), 0, false)
            .expect_err("lowered window is full");
        assert_eq!(limited.retry_at, Some(start + Duration::from_secs(62)));
    }

    fn rpm(value: u32) -> RequestsPerMinute {
        RequestsPerMinute::new(value).expect("valid RPM")
    }
}
