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

        // Coalesce racing advances: skip the wakeup when a concurrent advance
        // has already published an equal or newer epoch.
        self.sender.send_if_modified(|published| {
            if *published >= next {
                return false;
            }
            *published = next;
            true
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

    fn prepare_wake_schedule(
        self: &Arc<Self>,
        slot: u64,
        at: Instant,
    ) -> PendingSchedulerWakeNotification {
        if self.lifecycle.phase() != ShutdownPhase::Running {
            return PendingSchedulerWakeNotification::unchanged();
        }
        let changed = self
            .wake_schedule
            .lock()
            .expect("scheduler wake schedule lock poisoned")
            .schedule(slot, at);
        PendingSchedulerWakeNotification::new(Arc::clone(self), changed, true)
    }

    fn prepare_wake_cancellation(self: &Arc<Self>, slot: u64) -> PendingSchedulerWakeNotification {
        let changed = self
            .wake_schedule
            .lock()
            .expect("scheduler wake schedule lock poisoned")
            .cancel(slot);
        PendingSchedulerWakeNotification::new(Arc::clone(self), changed, false)
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
#[must_use = "publish scheduler wake notifications after releasing outer state locks"]
pub(crate) struct PendingSchedulerWakeNotification {
    scheduler: Option<Arc<SchedulerEpoch>>,
    ensure_worker: bool,
}

impl PendingSchedulerWakeNotification {
    fn new(scheduler: Arc<SchedulerEpoch>, changed: bool, ensure_worker: bool) -> Self {
        Self {
            scheduler: changed.then_some(scheduler),
            ensure_worker,
        }
    }

    fn unchanged() -> Self {
        Self {
            scheduler: None,
            ensure_worker: false,
        }
    }

    pub(crate) fn publish(self) {
        let Some(scheduler) = self.scheduler else {
            return;
        };
        scheduler.notify_wake_worker();
        if self.ensure_worker {
            scheduler.ensure_wake_worker();
        }
    }
}

#[derive(Debug)]
pub(crate) struct SchedulerWakeSlot {
    id: u64,
    scheduler: Arc<SchedulerEpoch>,
}

impl SchedulerWakeSlot {
    #[cfg(test)]
    pub(crate) fn schedule(&self, at: Instant) {
        self.prepare_schedule(at).publish();
    }

    pub(crate) fn prepare_schedule(&self, at: Instant) -> PendingSchedulerWakeNotification {
        self.scheduler.prepare_wake_schedule(self.id, at)
    }

    pub(crate) fn cancel(&self) {
        self.prepare_cancellation().publish();
    }

    pub(crate) fn prepare_cancellation(&self) -> PendingSchedulerWakeNotification {
        self.scheduler.prepare_wake_cancellation(self.id)
    }
}

impl Drop for SchedulerWakeSlot {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests;
