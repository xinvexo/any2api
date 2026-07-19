use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::scheduler_epoch::SchedulerEpoch;
use tokio::time::Instant;

#[derive(Debug)]
pub(super) struct CircuitRuntime {
    state: Mutex<CircuitState>,
    scheduler_epoch: Arc<SchedulerEpoch>,
}

#[derive(Debug)]
struct CircuitState {
    failure_times: VecDeque<Instant>,
    open_until: Option<Instant>,
    half_open_in_flight: u32,
    last_failure_at: Option<Instant>,
}

impl CircuitRuntime {
    pub(super) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(CircuitState {
                failure_times: VecDeque::new(),
                open_until: None,
                half_open_in_flight: 0,
                last_failure_at: None,
            }),
            scheduler_epoch,
        })
    }

    pub(super) fn try_acquire(
        self: &Arc<Self>,
        half_open_max_probes: u32,
    ) -> Result<CircuitPermit, Instant> {
        let now = Instant::now();
        let mut state = self.state.lock().expect("circuit state lock poisoned");
        if let Some(open_until) = state.open_until {
            if now < open_until {
                return Err(open_until);
            }
            if state.half_open_in_flight >= half_open_max_probes {
                return Err(now + Duration::from_millis(10));
            }
            state.half_open_in_flight += 1;
            return Ok(CircuitPermit {
                runtime: Arc::clone(self),
                half_open: true,
                resolved: false,
                started_at: now,
            });
        }
        Ok(CircuitPermit {
            runtime: Arc::clone(self),
            half_open: false,
            resolved: false,
            started_at: now,
        })
    }

    pub(super) fn availability(&self, half_open_max_probes: u32) -> Result<(), Instant> {
        let now = Instant::now();
        let state = self.state.lock().expect("circuit state lock poisoned");
        match state.open_until {
            Some(open_until) if now < open_until => Err(open_until),
            Some(_) if state.half_open_in_flight >= half_open_max_probes => {
                Err(now + Duration::from_millis(10))
            }
            _ => Ok(()),
        }
    }

    fn success(&self, half_open: bool, started_at: Instant) {
        let mut state = self.state.lock().expect("circuit state lock poisoned");
        if half_open {
            state.half_open_in_flight = state.half_open_in_flight.saturating_sub(1);
        }
        if state
            .last_failure_at
            .is_some_and(|failed_at| failed_at > started_at)
        {
            return;
        }
        let changed = !state.failure_times.is_empty() || state.open_until.is_some();
        state.failure_times.clear();
        state.open_until = None;
        state.last_failure_at = None;
        drop(state);
        if changed {
            self.scheduler_epoch.advance();
        }
    }

    fn failure(
        &self,
        half_open: bool,
        threshold: u32,
        failure_window: Duration,
        open_duration: Duration,
    ) -> Option<Instant> {
        let now = Instant::now();
        let mut state = self.state.lock().expect("circuit state lock poisoned");
        if half_open {
            state.half_open_in_flight = state.half_open_in_flight.saturating_sub(1);
        }
        while state
            .failure_times
            .front()
            .is_some_and(|failed_at| now.saturating_duration_since(*failed_at) >= failure_window)
        {
            state.failure_times.pop_front();
        }
        state.failure_times.push_back(now);
        state.last_failure_at = Some(now);
        let should_open = half_open || state.failure_times.len() >= threshold as usize;
        let open_until = should_open.then_some(now + open_duration);
        if let Some(open_until) = open_until {
            state.open_until = Some(open_until);
            state.failure_times.clear();
        }
        drop(state);
        if let Some(open_until) = open_until {
            self.schedule_wake(open_until);
        }
        open_until
    }

    fn release_probe(&self) {
        let mut state = self.state.lock().expect("circuit state lock poisoned");
        state.half_open_in_flight = state.half_open_in_flight.saturating_sub(1);
        drop(state);
        self.scheduler_epoch.advance();
    }

    fn schedule_wake(&self, wake_at: Instant) {
        let epoch = Arc::clone(&self.scheduler_epoch);
        tokio::spawn(async move {
            tokio::time::sleep_until(wake_at).await;
            epoch.advance();
        });
    }
}

pub(super) struct CircuitPermit {
    runtime: Arc<CircuitRuntime>,
    half_open: bool,
    resolved: bool,
    started_at: Instant,
}

impl CircuitPermit {
    pub(super) fn success(mut self) {
        self.runtime.success(self.half_open, self.started_at);
        self.resolved = true;
    }

    pub(super) fn failure(
        mut self,
        threshold: u32,
        window: Duration,
        open_duration: Duration,
    ) -> Option<Instant> {
        let open_until = self
            .runtime
            .failure(self.half_open, threshold, window, open_duration);
        self.resolved = true;
        open_until
    }
}

impl Drop for CircuitPermit {
    fn drop(&mut self) {
        if self.half_open && !self.resolved {
            self.runtime.release_probe();
        }
    }
}
