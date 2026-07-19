use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use thiserror::Error;
use tokio::sync::watch;

use crate::scheduler_epoch::SchedulerEpoch;

const DEFAULT_QUEUE_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_WAITING_REQUESTS: u32 = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaturationAction {
    Wait,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueuePolicy {
    on_saturated: SaturationAction,
    queue_timeout: Duration,
    max_waiting_requests: u32,
    fallback_on_saturation: bool,
}

impl QueuePolicy {
    pub const fn new(
        on_saturated: SaturationAction,
        queue_timeout: Duration,
        max_waiting_requests: u32,
        fallback_on_saturation: bool,
    ) -> Result<Self, QueuePolicyError> {
        if queue_timeout.is_zero() {
            return Err(QueuePolicyError::ZeroQueueTimeout);
        }
        if max_waiting_requests == 0 {
            return Err(QueuePolicyError::ZeroMaxWaitingRequests);
        }
        Ok(Self {
            on_saturated,
            queue_timeout,
            max_waiting_requests,
            fallback_on_saturation,
        })
    }

    #[must_use]
    pub const fn on_saturated(self) -> SaturationAction {
        self.on_saturated
    }

    #[must_use]
    pub const fn queue_timeout(self) -> Duration {
        self.queue_timeout
    }

    #[must_use]
    pub const fn max_waiting_requests(self) -> u32 {
        self.max_waiting_requests
    }

    #[must_use]
    pub const fn fallback_on_saturation(self) -> bool {
        self.fallback_on_saturation
    }
}

impl Default for QueuePolicy {
    fn default() -> Self {
        Self {
            on_saturated: SaturationAction::Wait,
            queue_timeout: DEFAULT_QUEUE_TIMEOUT,
            max_waiting_requests: DEFAULT_MAX_WAITING_REQUESTS,
            fallback_on_saturation: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum QueuePolicyError {
    #[error("queue timeout must be greater than zero")]
    ZeroQueueTimeout,
    #[error("maximum waiting requests must be greater than zero")]
    ZeroMaxWaitingRequests,
}

#[derive(Debug, Default)]
struct QueueState {
    waiting: u32,
}

#[derive(Debug)]
pub(crate) struct QueueCoordinator {
    state: Mutex<QueueState>,
    scheduler_epoch: Arc<SchedulerEpoch>,
}

impl QueueCoordinator {
    pub(crate) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(QueueState::default()),
            scheduler_epoch,
        })
    }

    pub(crate) fn try_ticket(self: &Arc<Self>, max_waiting_requests: u32) -> Option<QueueTicket> {
        let mut state = self.state.lock().expect("queue state lock poisoned");
        if state.waiting >= max_waiting_requests {
            return None;
        }
        state.waiting += 1;
        drop(state);

        Some(QueueTicket {
            coordinator: Arc::clone(self),
            receiver: self.scheduler_epoch.subscribe(),
        })
    }

    pub(crate) fn waiting_count(&self) -> u32 {
        self.state
            .lock()
            .expect("queue state lock poisoned")
            .waiting
    }
}

pub(crate) struct QueueTicket {
    coordinator: Arc<QueueCoordinator>,
    receiver: watch::Receiver<u64>,
}

impl QueueTicket {
    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.receiver.clone()
    }
}

impl Drop for QueueTicket {
    fn drop(&mut self) {
        let mut state = self
            .coordinator
            .state
            .lock()
            .expect("queue state lock poisoned");
        state.waiting = state
            .waiting
            .checked_sub(1)
            .expect("queue ticket released without a waiting request");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{QueueCoordinator, QueuePolicy, QueuePolicyError, SaturationAction};
    use crate::scheduler_epoch::SchedulerEpoch;

    #[test]
    fn default_policy_matches_the_architecture_defaults() {
        let policy = QueuePolicy::default();
        assert_eq!(policy.on_saturated(), SaturationAction::Wait);
        assert_eq!(policy.queue_timeout().as_secs(), 30);
        assert_eq!(policy.max_waiting_requests(), 128);
        assert!(!policy.fallback_on_saturation());
    }

    #[test]
    fn policy_rejects_zero_limits() {
        assert_eq!(
            QueuePolicy::new(SaturationAction::Wait, std::time::Duration::ZERO, 1, false,),
            Err(QueuePolicyError::ZeroQueueTimeout)
        );
        assert_eq!(
            QueuePolicy::new(
                SaturationAction::Wait,
                std::time::Duration::from_secs(1),
                0,
                false,
            ),
            Err(QueuePolicyError::ZeroMaxWaitingRequests)
        );
    }

    #[test]
    fn tickets_are_bounded_and_release_their_waiting_slot_once() {
        let coordinator = QueueCoordinator::new(SchedulerEpoch::new());
        let first = coordinator.try_ticket(1).expect("first ticket");
        assert_eq!(coordinator.waiting_count(), 1);
        assert!(coordinator.try_ticket(1).is_none());
        drop(first);
        assert_eq!(coordinator.waiting_count(), 0);
        assert!(coordinator.try_ticket(1).is_some());
    }

    #[test]
    fn ticket_subscription_starts_at_the_current_epoch() {
        let epoch = SchedulerEpoch::new();
        epoch.advance();
        let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
        let ticket = coordinator.try_ticket(1).expect("ticket");
        let receiver = ticket.subscribe();
        assert_eq!(*receiver.borrow(), 1);
    }
}
