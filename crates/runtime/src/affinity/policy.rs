use std::time::Duration;

use any2api_domain::{AffinityMode, AffinitySettings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffinityPolicy {
    soft_enabled: bool,
    soft_mode: AffinityMode,
    soft_ttl: Duration,
    hard_ttl: Duration,
    soft_prefer_wait_timeout: Duration,
    fixed_wait_timeout: Duration,
}

impl AffinityPolicy {
    pub(crate) fn from_settings(settings: &AffinitySettings) -> Self {
        Self {
            soft_enabled: settings.soft_enabled(),
            soft_mode: settings.soft_mode(),
            soft_ttl: Duration::from_millis(settings.soft_ttl_ms()),
            hard_ttl: Duration::from_millis(settings.hard_ttl_ms()),
            soft_prefer_wait_timeout: Duration::from_millis(settings.soft_prefer_wait_timeout_ms()),
            fixed_wait_timeout: Duration::from_millis(settings.fixed_wait_timeout_ms()),
        }
    }

    pub const fn soft_enabled(self) -> bool {
        self.soft_enabled
    }

    pub const fn soft_mode(self) -> AffinityMode {
        self.soft_mode
    }

    pub const fn soft_ttl(self) -> Duration {
        self.soft_ttl
    }

    pub const fn hard_ttl(self) -> Duration {
        self.hard_ttl
    }

    pub const fn soft_prefer_wait_timeout(self) -> Duration {
        self.soft_prefer_wait_timeout
    }

    pub const fn fixed_wait_timeout(self) -> Duration {
        self.fixed_wait_timeout
    }
}
