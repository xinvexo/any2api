use std::time::Duration;

use any2api_domain::AffinitySettings;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffinityPolicy {
    ttl: Duration,
    wait_timeout: Duration,
}

impl AffinityPolicy {
    pub(crate) fn from_settings(settings: &AffinitySettings) -> Self {
        Self {
            ttl: Duration::from_secs(settings.ttl_secs()),
            wait_timeout: Duration::from_secs(settings.wait_timeout_secs()),
        }
    }

    pub const fn ttl(self) -> Duration {
        self.ttl
    }

    pub const fn wait_timeout(self) -> Duration {
        self.wait_timeout
    }
}
