use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use tokio::sync::watch;

#[derive(Debug)]
pub(crate) struct SchedulerEpoch {
    current: AtomicU64,
    sender: watch::Sender<u64>,
}

impl SchedulerEpoch {
    pub(crate) fn new() -> Arc<Self> {
        let (sender, _receiver) = watch::channel(0);
        Arc::new(Self {
            current: AtomicU64::new(0),
            sender,
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
}
