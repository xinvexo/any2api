use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use any2api_domain::PublicErrorCode;
use tokio::time::Instant;

use super::{candidate, try_reserve_candidate};
use crate::{
    public_request::selection::{
        GenerationSelection, select_generation_candidate, wait_for_generation_candidate,
    },
    routing::{QueueCoordinator, QueuePolicy, RateLimitAction, SchedulerEpoch},
};

#[tokio::test]
async fn reject_policy_does_not_enter_the_queue() {
    let coordinator = QueueCoordinator::new(SchedulerEpoch::new());
    let policy = policy(RateLimitAction::Reject, Duration::from_secs(1), 1);

    let error = match select_generation_candidate(&coordinator, policy, || {
        Ok(GenerationSelection::RateLimited(None))
    })
    .await
    {
        Ok(_) => panic!("reject policy must fail immediately"),
        Err(error) => error,
    };

    assert_eq!(error.code(), PublicErrorCode::LocalRateLimit);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test]
async fn queue_limit_rejects_an_additional_waiter() {
    let coordinator = QueueCoordinator::new(SchedulerEpoch::new());
    let occupied = coordinator.try_ticket(1).expect("occupied ticket");
    let policy = policy(RateLimitAction::Wait, Duration::from_secs(1), 1);

    let error = match select_generation_candidate(&coordinator, policy, || {
        Ok(GenerationSelection::RateLimited(None))
    })
    .await
    {
        Ok(_) => panic!("bounded queue must reject another waiter"),
        Err(error) => error,
    };

    assert_eq!(error.code(), PublicErrorCode::LocalRateLimit);
    assert_eq!(error.client_message(), "request queue is full");
    assert_eq!(coordinator.waiting_count(), 1);
    drop(occupied);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn generation_wait_reselects_when_the_oldest_rpm_reservation_expires() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("queued", 1, Arc::clone(&epoch), 0);
    drop(candidate.binding.try_reserve().expect("exhaust RPM"));
    let queued_candidate = candidate.clone();
    let policy = policy(RateLimitAction::Wait, Duration::from_secs(61), 1);
    let coordinator_for_task = Arc::clone(&coordinator);
    let task = tokio::spawn(async move {
        wait_for_generation_candidate(&coordinator_for_task, policy, || {
            try_reserve_candidate(&queued_candidate)
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    tokio::time::advance(Duration::from_secs(60)).await;
    let selected = task.await.expect("queue task").expect("selected candidate");
    assert_eq!(selected.candidate.credential_id, candidate.credential_id);
    drop(selected);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn generation_wait_uses_health_retry_at_without_an_epoch_broadcast() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("health-deadline", 1, Arc::clone(&epoch), 0);
    let retry_at = Instant::now() + Duration::from_secs(5);
    let queued_candidate = candidate.clone();
    let policy = policy(RateLimitAction::Wait, Duration::from_secs(10), 1);
    let coordinator_for_task = Arc::clone(&coordinator);
    let task = tokio::spawn(async move {
        select_generation_candidate(&coordinator_for_task, policy, || {
            if Instant::now() < retry_at {
                Ok(GenerationSelection::TemporarilyUnavailable(retry_at))
            } else {
                try_reserve_candidate(&queued_candidate)
            }
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    tokio::time::advance(Duration::from_secs(5)).await;
    let selected = task.await.expect("queue task").expect("selected candidate");

    assert_eq!(epoch.current(), 0);
    assert_eq!(selected.candidate.credential_id, candidate.credential_id);
    drop(selected);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn generation_wait_times_out_and_releases_its_ticket() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("timeout", 1, Arc::clone(&epoch), 0);
    drop(candidate.binding.try_reserve().expect("exhaust RPM"));
    let policy = policy(RateLimitAction::Wait, Duration::from_secs(1), 1);
    let queued_candidate = candidate.clone();
    let coordinator_for_task = Arc::clone(&coordinator);
    let task = tokio::spawn(async move {
        wait_for_generation_candidate(&coordinator_for_task, policy, || {
            try_reserve_candidate(&queued_candidate)
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    tokio::time::advance(Duration::from_secs(1)).await;
    let error = match task.await.expect("queue task") {
        Ok(_) => panic!("queue must time out"),
        Err(error) => error,
    };

    assert_eq!(error.code(), PublicErrorCode::LocalRateLimit);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn timeout_boundary_performs_one_final_selection() {
    let coordinator = QueueCoordinator::new(SchedulerEpoch::new());
    let candidate = candidate("deadline", 1, SchedulerEpoch::new(), 0);
    let policy = policy(RateLimitAction::Wait, Duration::from_secs(1), 1);
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_task = Arc::clone(&attempts);
    let coordinator_for_task = Arc::clone(&coordinator);
    let candidate_for_task = candidate.clone();
    let task = tokio::spawn(async move {
        select_generation_candidate(&coordinator_for_task, policy, || {
            let attempt = attempts_for_task.fetch_add(1, Ordering::AcqRel) + 1;
            if attempt < 3 {
                Ok(GenerationSelection::RateLimited(None))
            } else {
                try_reserve_candidate(&candidate_for_task)
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
    let policy = policy(RateLimitAction::Wait, Duration::from_secs(1), 1);
    let mut attempts = 0_u8;

    let selected = wait_for_generation_candidate(&coordinator, policy, || {
        attempts += 1;
        if attempts == 1 {
            epoch.advance();
            Ok(GenerationSelection::RateLimited(None))
        } else {
            try_reserve_candidate(&candidate)
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
    drop(candidate.binding.try_reserve().expect("exhaust RPM"));
    let policy = policy(RateLimitAction::Wait, Duration::from_secs(30), 1);
    let queued_candidate = candidate.clone();
    let coordinator_for_task = Arc::clone(&coordinator);
    let task = tokio::spawn(async move {
        wait_for_generation_candidate(&coordinator_for_task, policy, || {
            try_reserve_candidate(&queued_candidate)
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    task.abort();
    assert!(task.await.is_err());
    assert_eq!(coordinator.waiting_count(), 0);
    assert_eq!(candidate.binding.in_flight(), 0);
    assert_eq!(candidate.binding.rate_snapshot().requests_in_window(), 1);
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

fn policy(action: RateLimitAction, timeout: Duration, max_waiting: u32) -> QueuePolicy {
    QueuePolicy::new(action, timeout, max_waiting, false).expect("queue policy")
}
