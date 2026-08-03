use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

const WARNING_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(super) struct IoDiagnostics {
    failing: bool,
    next_warning: Option<Instant>,
}

impl IoDiagnostics {
    pub(super) fn report_failure(&mut self, error: &io::Error) {
        if self.failure_warning_due(Instant::now()) {
            let _ = writeln!(
                io::stderr().lock(),
                "any2api file logging I/O failure; local logs may be incomplete: {error}"
            );
        }
    }

    pub(super) fn report_recovery(&mut self) {
        if self.finish_recovery() {
            let _ = writeln!(
                io::stderr().lock(),
                "any2api file logging recovered after an I/O failure"
            );
        }
    }

    fn failure_warning_due(&mut self, now: Instant) -> bool {
        let due = !self.failing || self.next_warning.is_none_or(|deadline| now >= deadline);
        self.failing = true;
        if due {
            self.next_warning = Some(now.checked_add(WARNING_INTERVAL).unwrap_or(now));
        }
        due
    }

    fn finish_recovery(&mut self) -> bool {
        let recovered = self.failing;
        self.failing = false;
        self.next_warning = None;
        recovered
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::IoDiagnostics;

    #[test]
    fn failure_warnings_are_immediate_then_limited_until_one_recovery() {
        let mut diagnostics = IoDiagnostics::default();
        let start = Instant::now();

        assert!(diagnostics.failure_warning_due(start));
        assert!(!diagnostics.failure_warning_due(start + Duration::from_secs(59)));
        assert!(diagnostics.failure_warning_due(start + Duration::from_secs(60)));
        assert!(diagnostics.finish_recovery());
        assert!(!diagnostics.finish_recovery());
        assert!(diagnostics.failure_warning_due(start + Duration::from_secs(61)));
    }
}
