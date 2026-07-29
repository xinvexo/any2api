use std::time::Duration;

use super::ShutdownTimeouts;

pub(crate) struct ShutdownOutcome {
    result: anyhow::Result<()>,
    timeouts: ShutdownTimeouts,
    fatal: bool,
    restart_requested: bool,
}

impl ShutdownOutcome {
    pub(crate) fn complete(result: anyhow::Result<()>, timeouts: ShutdownTimeouts) -> Self {
        Self {
            result,
            timeouts,
            fatal: false,
            restart_requested: false,
        }
    }

    pub(crate) fn fatal(error: anyhow::Error, timeouts: ShutdownTimeouts) -> Self {
        Self {
            result: Err(error),
            timeouts,
            fatal: true,
            restart_requested: false,
        }
    }

    pub(crate) fn runtime_shutdown_timeout(&self) -> Duration {
        self.timeouts.runtime_shutdown_timeout()
    }

    pub(crate) const fn is_fatal(&self) -> bool {
        self.fatal
    }

    pub(crate) const fn restart_requested(&self) -> bool {
        self.restart_requested
    }

    pub(crate) fn with_restart_requested(mut self, requested: bool) -> Self {
        self.restart_requested = requested;
        self
    }

    pub(crate) fn into_result(self) -> anyhow::Result<()> {
        self.result
    }
}
