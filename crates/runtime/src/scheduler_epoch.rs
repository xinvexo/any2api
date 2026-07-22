use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use tokio::sync::watch;
use tokio::time::Instant;

use crate::process_lifecycle::ProcessLifecycle;

#[derive(Debug)]
pub(crate) struct SchedulerEpoch {
    current: AtomicU64,
    sender: watch::Sender<u64>,
    lifecycle: ProcessLifecycle,
}

impl SchedulerEpoch {
    #[cfg(test)]
    pub(crate) fn new() -> Arc<Self> {
        Self::with_lifecycle(ProcessLifecycle::new())
    }

    pub(crate) fn with_lifecycle(lifecycle: ProcessLifecycle) -> Arc<Self> {
        let (sender, _receiver) = watch::channel(0);
        Arc::new(Self {
            current: AtomicU64::new(0),
            sender,
            lifecycle,
        })
    }

    pub(crate) fn current(&self) -> u64 {
        self.current.load(Ordering::Acquire)
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.sender.subscribe()
    }

    pub(crate) fn advance(&self) -> u64 {
        let mut current = self.current.load(Ordering::Acquire);
        let next = loop {
            let next = current
                .checked_add(1)
                .expect("scheduler epoch exhausted u64");
            match self.current.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break next,
                Err(observed) => current = observed,
            }
        };

        self.sender.send_modify(|published| {
            *published = (*published).max(next);
        });
        next
    }

    pub(crate) fn schedule_wake(self: &Arc<Self>, at: Instant) {
        let epoch = Arc::clone(self);
        self.lifecycle.spawn_until_draining(async move {
            tokio::time::sleep_until(at).await;
            epoch.advance();
        });
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::SchedulerEpoch;
    use crate::process_lifecycle::ProcessLifecycle;

    #[tokio::test(start_paused = true)]
    async fn scheduled_health_wake_stops_when_draining_begins() {
        let lifecycle = ProcessLifecycle::new();
        let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
        epoch.schedule_wake(Instant::now() + Duration::from_secs(60));
        assert_eq!(lifecycle.background_task_count(), 1);

        lifecycle.begin_draining();
        lifecycle.close_background_tasks();
        lifecycle.wait_for_background_tasks().await;

        assert_eq!(epoch.current(), 0);
        assert_eq!(lifecycle.background_task_count(), 0);
    }
}
