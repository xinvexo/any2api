mod schedule;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use tokio::sync::watch;
use tokio::time::{Instant, sleep_until};

use self::schedule::WakeSchedule;
use crate::lifecycle::{ProcessLifecycle, ShutdownPhase};

#[derive(Debug)]
pub(crate) struct SchedulerEpoch {
    current: AtomicU64,
    sender: watch::Sender<u64>,
    lifecycle: ProcessLifecycle,
    wake_schedule: Mutex<WakeSchedule>,
    wake_changes: watch::Sender<u64>,
    wake_worker_started: AtomicBool,
    next_wake_slot: AtomicU64,
}

impl SchedulerEpoch {
    #[cfg(test)]
    pub(crate) fn new() -> Arc<Self> {
        Self::with_lifecycle(ProcessLifecycle::new())
    }

    pub(crate) fn with_lifecycle(lifecycle: ProcessLifecycle) -> Arc<Self> {
        let (sender, _receiver) = watch::channel(0);
        let (wake_changes, _receiver) = watch::channel(0);
        Arc::new(Self {
            current: AtomicU64::new(0),
            sender,
            lifecycle,
            wake_schedule: Mutex::new(WakeSchedule::default()),
            wake_changes,
            wake_worker_started: AtomicBool::new(false),
            next_wake_slot: AtomicU64::new(1),
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

    pub(crate) fn wake_slot(self: &Arc<Self>) -> SchedulerWakeSlot {
        let id = self
            .next_wake_slot
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .expect("scheduler wake slot id exhausted u64");
        SchedulerWakeSlot {
            id,
            scheduler: Arc::clone(self),
        }
    }

    fn schedule_wake(self: &Arc<Self>, slot: u64, at: Instant) {
        if self.lifecycle.phase() != ShutdownPhase::Running {
            return;
        }
        let changed = self
            .wake_schedule
            .lock()
            .expect("scheduler wake schedule lock poisoned")
            .schedule(slot, at);
        if !changed {
            return;
        }
        self.notify_wake_worker();
        self.ensure_wake_worker();
    }

    fn cancel_wake(&self, slot: u64) {
        let changed = self
            .wake_schedule
            .lock()
            .expect("scheduler wake schedule lock poisoned")
            .cancel(slot);
        if changed {
            self.notify_wake_worker();
        }
    }

    fn notify_wake_worker(&self) {
        self.wake_changes.send_modify(|generation| {
            *generation = generation
                .checked_add(1)
                .expect("scheduler wake generation exhausted u64");
        });
    }

    fn ensure_wake_worker(self: &Arc<Self>) {
        if self.lifecycle.phase() != ShutdownPhase::Running
            || self
                .wake_worker_started
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return;
        }
        if self.lifecycle.phase() != ShutdownPhase::Running {
            self.wake_worker_started.store(false, Ordering::Release);
            return;
        }
        let scheduler = Arc::clone(self);
        drop(self.lifecycle.spawn_until_draining(async move {
            scheduler.run_wake_worker().await;
        }));
    }

    async fn run_wake_worker(&self) {
        let mut changes = self.wake_changes.subscribe();
        loop {
            let _observed_generation = *changes.borrow_and_update();
            let next = self
                .wake_schedule
                .lock()
                .expect("scheduler wake schedule lock poisoned")
                .next_deadline();
            let Some(next) = next else {
                if changes.changed().await.is_err() {
                    return;
                }
                continue;
            };
            tokio::select! {
                changed = changes.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                () = sleep_until(next) => {
                    let removed = self
                        .wake_schedule
                        .lock()
                        .expect("scheduler wake schedule lock poisoned")
                        .remove_due(Instant::now());
                    if removed {
                        self.advance();
                    }
                }
            }
        }
    }

    #[cfg(test)]
    fn scheduled_wake_count(&self) -> usize {
        self.wake_schedule
            .lock()
            .expect("scheduler wake schedule lock poisoned")
            .len()
    }
}

#[derive(Debug)]
pub(crate) struct SchedulerWakeSlot {
    id: u64,
    scheduler: Arc<SchedulerEpoch>,
}

impl SchedulerWakeSlot {
    pub(crate) fn schedule(&self, at: Instant) {
        self.scheduler.schedule_wake(self.id, at);
    }

    pub(crate) fn cancel(&self) {
        self.scheduler.cancel_wake(self.id);
    }
}

impl Drop for SchedulerWakeSlot {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests;
