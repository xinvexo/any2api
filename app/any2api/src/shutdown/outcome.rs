use std::time::Duration;

use super::ShutdownTimeouts;

pub(crate) struct ShutdownOutcome {
    result: anyhow::Result<()>,
    timeouts: ShutdownTimeouts,
    fatal: bool,
    restart_requested: bool,
}

impl ShutdownOutcome {
    pub(crate) fn after_finalization(
        server_result: anyhow::Result<()>,
        finalization: anyhow::Result<()>,
        timeouts: ShutdownTimeouts,
    ) -> Self {
        match finalization {
            Ok(()) => Self {
                result: server_result,
                timeouts,
                fatal: false,
                restart_requested: false,
            },
            Err(error) => Self {
                result: Err(error),
                timeouts,
                fatal: true,
                restart_requested: false,
            },
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

#[cfg(test)]
mod tests {
    use super::{ShutdownOutcome, ShutdownTimeouts};

    #[test]
    fn restart_request_survives_successful_critical_finalization() {
        let outcome =
            ShutdownOutcome::after_finalization(Ok(()), Ok(()), ShutdownTimeouts::defaults())
                .with_restart_requested(true);

        assert!(!outcome.is_fatal());
        assert!(outcome.restart_requested());
        outcome.into_result().expect("successful shutdown");
    }

    #[test]
    fn critical_finalization_failure_remains_fatal_even_with_restart_requested() {
        let outcome = ShutdownOutcome::after_finalization(
            Ok(()),
            Err(anyhow::anyhow!("sqlite did not close")),
            ShutdownTimeouts::defaults(),
        )
        .with_restart_requested(true);

        assert!(outcome.is_fatal());
        assert!(outcome.restart_requested());
        assert!(outcome.into_result().is_err());
    }
}
