use std::time::Duration;

use super::ShutdownTimeouts;

pub(crate) struct ShutdownOutcome {
    result: anyhow::Result<()>,
    timeouts: ShutdownTimeouts,
    fatal: bool,
}

impl ShutdownOutcome {
    pub(crate) fn complete(result: anyhow::Result<()>, timeouts: ShutdownTimeouts) -> Self {
        Self {
            result,
            timeouts,
            fatal: false,
        }
    }

    pub(crate) fn fatal(error: anyhow::Error, timeouts: ShutdownTimeouts) -> Self {
        Self {
            result: Err(error),
            timeouts,
            fatal: true,
        }
    }

    pub(crate) fn runtime_shutdown_timeout(&self) -> Duration {
        self.timeouts.runtime_shutdown_timeout()
    }

    pub(crate) const fn is_fatal(&self) -> bool {
        self.fatal
    }

    pub(crate) fn into_result(self) -> anyhow::Result<()> {
        self.result
    }
}
