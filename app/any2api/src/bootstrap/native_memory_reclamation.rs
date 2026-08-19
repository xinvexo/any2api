use std::time::Duration;

use any2api_memory_reclaimer::relieve_native_allocator_pressure;
use any2api_runtime::api::ProcessLifecycle;
use tokio::time::{Instant, sleep_until};

const RECLAIM_COOLDOWN: Duration = Duration::from_secs(30);

pub(super) fn start(lifecycle: &ProcessLifecycle) {
    let worker_lifecycle = lifecycle.clone();
    drop(lifecycle.spawn_critical(run(worker_lifecycle)));
}

async fn run(lifecycle: ProcessLifecycle) {
    let mut state = ReclaimState::default();
    let mut next_reclaim_at = Instant::now();

    loop {
        tokio::select! {
            () = lifecycle.draining() => break,
            () = lifecycle.memory_reclamation_requested() => {}
        }
        tokio::select! {
            () = lifecycle.draining() => break,
            () = sleep_until(next_reclaim_at) => {}
        }
        let Some(activity_epoch) = state.pending_activity(
            lifecycle.activity_epoch(),
            lifecycle.memory_reclamation_blockers(),
        ) else {
            continue;
        };
        let started = Instant::now();
        match lifecycle
            .spawn_blocking(relieve_native_allocator_pressure)
            .await
        {
            Ok(()) => {
                lifecycle.record_memory_reclamation(started.elapsed());
                state.record_reclaimed(activity_epoch);
                next_reclaim_at = Instant::now() + RECLAIM_COOLDOWN;
            }
            Err(error) => {
                tracing::debug!(%error, "native allocator pressure relief did not complete");
            }
        }
    }
}

#[derive(Default)]
struct ReclaimState {
    last_reclaimed_activity: u64,
}

impl ReclaimState {
    fn pending_activity(&self, activity_epoch: u64, reclamation_blockers: usize) -> Option<u64> {
        if reclamation_blockers != 0 || activity_epoch == self.last_reclaimed_activity {
            return None;
        }
        Some(activity_epoch)
    }

    fn record_reclaimed(&mut self, activity_epoch: u64) {
        self.last_reclaimed_activity = activity_epoch;
    }
}

#[cfg(test)]
mod tests {
    use super::ReclaimState;

    #[test]
    fn exposes_each_activity_epoch_once_without_reclamation_blockers() {
        let mut state = ReclaimState::default();
        assert_eq!(state.pending_activity(0, 0), None);
        assert_eq!(state.pending_activity(1, 1), None);
        assert_eq!(state.pending_activity(1, 0), Some(1));
        state.record_reclaimed(1);
        assert_eq!(state.pending_activity(1, 0), None);
        assert_eq!(state.pending_activity(2, 0), Some(2));
    }
}
