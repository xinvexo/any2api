use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use any2api_domain::PublicErrorCode;

use super::{candidate, try_acquire_candidate};
use crate::{
    public_request::selection::{
        GenerationSelection, select_generation_candidate, wait_for_generation_candidate,
    },
    queue::{QueueCoordinator, QueuePolicy, SaturationAction},
    scheduler_epoch::SchedulerEpoch,
};

#[tokio::test]
async fn reject_policy_does_not_enter_the_queue() {
    let coordinator = QueueCoordinator::new(SchedulerEpoch::new());
    let policy = policy(SaturationAction::Reject, Duration::from_secs(1), 1);

    let error = match select_generation_candidate(&coordinator, policy, || {
        Ok(GenerationSelection::AtCapacity)
    })
    .await
    {
        Ok(_) => panic!("reject policy must fail immediately"),
        Err(error) => error,
    };

    assert_eq!(error.code, PublicErrorCode::LocalConcurrencyLimit);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test]
async fn queue_limit_rejects_an_additional_waiter() {
    let coordinator = QueueCoordinator::new(SchedulerEpoch::new());
    let occupied = coordinator.try_ticket(1).expect("occupied ticket");
    let policy = policy(SaturationAction::Wait, Duration::from_secs(1), 1);

    let error = match select_generation_candidate(&coordinator, policy, || {
        Ok(GenerationSelection::AtCapacity)
    })
    .await
    {
        Ok(_) => panic!("bounded queue must reject another waiter"),
        Err(error) => error,
    };

    assert_eq!(error.code, PublicErrorCode::LocalConcurrencyLimit);
    assert_eq!(error.message, "request queue is full");
    assert_eq!(coordinator.waiting_count(), 1);
    drop(occupied);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test]
async fn generation_wait_reselects_after_a_permit_is_released() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("queued", 1, Arc::clone(&epoch), 0);
    let blocker = candidate.binding.try_acquire().expect("blocker permit");
    let queued_candidate = candidate.clone();
    let policy = policy(SaturationAction::Wait, Duration::from_secs(1), 1);
    let coordinator_for_task = Arc::clone(&coordinator);
    let task = tokio::spawn(async move {
        wait_for_generation_candidate(&coordinator_for_task, policy, || {
            try_acquire_candidate(&queued_candidate)
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    drop(blocker);
    let selected = task.await.expect("queue task").expect("selected candidate");
    assert_eq!(selected.candidate.credential_id, candidate.credential_id);
    drop(selected);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn generation_wait_times_out_and_releases_its_ticket() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("timeout", 1, Arc::clone(&epoch), 0);
    let blocker = candidate.binding.try_acquire().expect("blocker permit");
    let policy = policy(SaturationAction::Wait, Duration::from_secs(1), 1);
    let queued_candidate = candidate.clone();
    let coordinator_for_task = Arc::clone(&coordinator);
    let task = tokio::spawn(async move {
        wait_for_generation_candidate(&coordinator_for_task, policy, || {
            try_acquire_candidate(&queued_candidate)
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(1)).await;
    let error = match task.await.expect("queue task") {
        Ok(_) => panic!("queue must time out"),
        Err(error) => error,
    };

    assert_eq!(error.code, PublicErrorCode::LocalConcurrencyLimit);
    assert_eq!(coordinator.waiting_count(), 0);
    drop(blocker);
}

#[tokio::test(start_paused = true)]
async fn timeout_boundary_performs_one_final_selection() {
    let coordinator = QueueCoordinator::new(SchedulerEpoch::new());
    let candidate = candidate("deadline", 1, SchedulerEpoch::new(), 0);
    let policy = policy(SaturationAction::Wait, Duration::from_secs(1), 1);
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_task = Arc::clone(&attempts);
    let coordinator_for_task = Arc::clone(&coordinator);
    let candidate_for_task = candidate.clone();
    let task = tokio::spawn(async move {
        select_generation_candidate(&coordinator_for_task, policy, || {
            let attempt = attempts_for_task.fetch_add(1, Ordering::AcqRel) + 1;
            if attempt < 3 {
                Ok(GenerationSelection::AtCapacity)
            } else {
                try_acquire_candidate(&candidate_for_task)
            }
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    for _ in 0..100 {
        if attempts.load(Ordering::Acquire) >= 2 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(attempts.load(Ordering::Acquire), 2);
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(1)).await;
    let selected = task
        .await
        .expect("queue task")
        .expect("final selection at the timeout boundary");

    assert_eq!(attempts.load(Ordering::Acquire), 3);
    assert_eq!(selected.candidate.credential_id, candidate.credential_id);
    drop(selected);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test]
async fn epoch_advance_between_recheck_and_wait_is_not_lost() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("wake", 1, Arc::clone(&epoch), 0);
    let policy = policy(SaturationAction::Wait, Duration::from_secs(1), 1);
    let mut attempts = 0_u8;

    let selected = wait_for_generation_candidate(&coordinator, policy, || {
        attempts += 1;
        if attempts == 1 {
            epoch.advance();
            Ok(GenerationSelection::AtCapacity)
        } else {
            try_acquire_candidate(&candidate)
        }
    })
    .await
    .expect("epoch notification must wake the waiter");

    assert_eq!(attempts, 2);
    drop(selected);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test]
async fn cancellation_drops_the_queue_ticket() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("cancel", 1, Arc::clone(&epoch), 0);
    let blocker = candidate.binding.try_acquire().expect("blocker permit");
    let policy = policy(SaturationAction::Wait, Duration::from_secs(30), 1);
    let queued_candidate = candidate.clone();
    let coordinator_for_task = Arc::clone(&coordinator);
    let task = tokio::spawn(async move {
        wait_for_generation_candidate(&coordinator_for_task, policy, || {
            try_acquire_candidate(&queued_candidate)
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    task.abort();
    assert!(task.await.is_err());
    assert_eq!(coordinator.waiting_count(), 0);
    assert_eq!(candidate.binding.capacity().in_flight(), 1);
    drop(blocker);
}

async fn wait_until_waiting(coordinator: &QueueCoordinator, expected: u32) {
    for _ in 0..10_000 {
        if coordinator.waiting_count() == expected {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("queue task did not start");
}

fn policy(action: SaturationAction, timeout: Duration, max_waiting: u32) -> QueuePolicy {
    QueuePolicy::new(action, timeout, max_waiting, false).expect("queue policy")
}
