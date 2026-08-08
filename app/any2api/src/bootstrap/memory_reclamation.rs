use std::time::Duration;

use any2api_memory_reclaimer::reclaim_process_memory;
use any2api_runtime::api::ProcessLifecycle;
use tokio::time::{MissedTickBehavior, interval};

const RECLAIM_INTERVAL: Duration = Duration::from_secs(30);

pub(super) fn start(lifecycle: &ProcessLifecycle) {
    let worker_lifecycle = lifecycle.clone();
    drop(lifecycle.spawn_critical(run(worker_lifecycle)));
}

async fn run(lifecycle: ProcessLifecycle) {
    let mut ticker = interval(RECLAIM_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker.tick().await;
    let mut state = ReclaimState::default();

    loop {
        tokio::select! {
            () = lifecycle.draining() => break,
            _ = ticker.tick() => {
                if !state.should_reclaim(
                    lifecycle.request_activity_epoch(),
                    lifecycle.active_requests(),
                ) {
                    continue;
                }
                if let Err(error) = lifecycle.spawn_blocking(reclaim_process_memory).await {
                    tracing::debug!(%error, "process memory reclamation task did not complete");
                }
            }
        }
    }
}

#[derive(Default)]
struct ReclaimState {
    last_reclaimed_activity: u64,
}

impl ReclaimState {
    fn should_reclaim(&mut self, request_activity: u64, active_requests: usize) -> bool {
        if active_requests != 0 || request_activity == self.last_reclaimed_activity {
            return false;
        }
        self.last_reclaimed_activity = request_activity;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::ReclaimState;

    #[test]
    fn reclaims_each_activity_epoch_once_and_only_while_idle() {
        let mut state = ReclaimState::default();
        assert!(!state.should_reclaim(0, 0));
        assert!(!state.should_reclaim(1, 1));
        assert!(state.should_reclaim(1, 0));
        assert!(!state.should_reclaim(1, 0));
        assert!(state.should_reclaim(2, 0));
    }
}
