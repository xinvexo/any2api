use std::{sync::Arc, time::Duration};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect, PublicErrorCode, RouteTargetId};
use tokio::time::Instant;

use super::{
    SessionQueueWait, binding_wait_timeout, ensure_queue_wait, observe_epoch,
    selection_coordination_timeout, wait_for_attempt,
};
use crate::{
    affinity::{AffinityRegistry, AffinityTarget, BindingCreationPhase, BindingStart},
    routing::{QueueCoordinator, SchedulerEpoch},
};

const TTL: Duration = Duration::from_secs(120);
const WAIT_TIMEOUT: Duration = Duration::from_secs(1);

#[tokio::test]
async fn creating_wait_respects_the_global_queue_limit() {
    let epoch = SchedulerEpoch::new();
    let registry = AffinityRegistry::with_scheduler_epoch(Arc::clone(&epoch));
    let queue = QueueCoordinator::new(epoch);
    let occupied = queue.try_ticket(1).expect("occupied queue slot");
    let route_id = ModelRouteId::new();
    let _creator = create_attempting_session(&registry, route_id, "queue-full");

    let error =
        wait_for_session_binding(&registry, &queue, route_id, "queue-full", WAIT_TIMEOUT, 1)
            .await
            .expect_err("an additional creating waiter must be rejected");

    assert_eq!(error.code(), PublicErrorCode::LocalRateLimit);
    assert_eq!(error.client_message(), "request queue is full");
    assert_eq!(queue.waiting_count(), 1);
    drop(occupied);
    assert_eq!(queue.waiting_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn creating_wait_timeout_releases_its_queue_ticket() {
    let epoch = SchedulerEpoch::new();
    let registry = AffinityRegistry::with_scheduler_epoch(Arc::clone(&epoch));
    let queue = QueueCoordinator::new(epoch);
    let route_id = ModelRouteId::new();
    let _creator = create_attempting_session(&registry, route_id, "timeout");
    let task = spawn_waiter(
        Arc::clone(&registry),
        Arc::clone(&queue),
        route_id,
        "timeout",
    );

    wait_until_waiting(&queue, 1).await;
    tokio::time::advance(WAIT_TIMEOUT).await;
    let error = task
        .await
        .expect("waiter task")
        .expect_err("creating wait must time out");

    assert_eq!(error.code(), PublicErrorCode::LocalRateLimit);
    assert_eq!(error.client_message(), "session binding creation timed out");
    assert_eq!(queue.waiting_count(), 0);
}

#[tokio::test]
async fn cancelling_a_creating_wait_releases_its_queue_ticket() {
    let epoch = SchedulerEpoch::new();
    let registry = AffinityRegistry::with_scheduler_epoch(Arc::clone(&epoch));
    let queue = QueueCoordinator::new(epoch);
    let route_id = ModelRouteId::new();
    let _creator = create_attempting_session(&registry, route_id, "cancel");
    let task = spawn_waiter(
        Arc::clone(&registry),
        Arc::clone(&queue),
        route_id,
        "cancel",
    );

    wait_until_waiting(&queue, 1).await;
    task.abort();
    assert!(task.await.is_err());
    assert_eq!(queue.waiting_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn repeated_creator_wakes_do_not_extend_the_wait_deadline() {
    let epoch = SchedulerEpoch::new();
    let registry = AffinityRegistry::with_scheduler_epoch(Arc::clone(&epoch));
    let queue = QueueCoordinator::new(epoch);
    let route_id = ModelRouteId::new();
    let first_creator = create_attempting_session(&registry, route_id, "recreated");
    let task = spawn_waiter(
        Arc::clone(&registry),
        Arc::clone(&queue),
        route_id,
        "recreated",
    );

    wait_until_waiting(&queue, 1).await;
    tokio::time::advance(Duration::from_millis(500)).await;
    drop(first_creator);
    let _second_creator = create_attempting_session(&registry, route_id, "recreated");
    tokio::task::yield_now().await;
    assert_eq!(queue.waiting_count(), 1);

    tokio::time::advance(Duration::from_millis(500)).await;
    let error = task
        .await
        .expect("waiter task")
        .expect_err("the original absolute deadline must still apply");
    assert_eq!(error.client_message(), "session binding creation timed out");
    assert_eq!(queue.waiting_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn creator_commit_at_the_wait_deadline_does_not_bypass_timeout() {
    let epoch = SchedulerEpoch::new();
    let registry = AffinityRegistry::with_scheduler_epoch(Arc::clone(&epoch));
    let queue = QueueCoordinator::new(epoch);
    let route_id = ModelRouteId::new();
    let creator = create_attempting_session(&registry, route_id, "deadline-commit");
    let task = spawn_waiter(
        Arc::clone(&registry),
        Arc::clone(&queue),
        route_id,
        "deadline-commit",
    );

    wait_until_waiting(&queue, 1).await;
    tokio::time::advance(WAIT_TIMEOUT).await;
    creator
        .commit(target(route_id))
        .expect("creator commits at deadline");
    let error = task
        .await
        .expect("waiter task")
        .expect_err("elapsed binding wait deadline must win");

    assert_eq!(error.client_message(), "session binding creation timed out");
    assert_eq!(queue.waiting_count(), 0);
}

async fn wait_for_session_binding(
    registry: &Arc<AffinityRegistry>,
    queue: &Arc<QueueCoordinator>,
    route_id: ModelRouteId,
    raw: &str,
    wait_timeout: Duration,
    max_waiting_requests: u32,
) -> Result<AffinityTarget, any2api_domain::PublicError> {
    let deadline = Instant::now() + wait_timeout;
    let mut wait: Option<SessionQueueWait> = None;
    let mut waiting_on_attempt = false;
    loop {
        if waiting_on_attempt && Instant::now() >= deadline {
            return Err(binding_wait_timeout());
        }
        waiting_on_attempt = false;
        let _observed_epoch = observe_epoch(&mut wait);
        match registry
            .begin_session(ProtocolDialect::OpenAiResponses, route_id, raw, TTL)
            .map_err(super::super::affinity_error)?
        {
            BindingStart::Create(lease) => {
                drop(lease);
                return Err(selection_coordination_timeout());
            }
            BindingStart::Wait(BindingCreationPhase::Selecting) => {
                return Err(selection_coordination_timeout());
            }
            BindingStart::Wait(BindingCreationPhase::Attempting) => {
                if Instant::now() >= deadline {
                    return Err(binding_wait_timeout());
                }
                if ensure_queue_wait(&mut wait, queue, max_waiting_requests)? {
                    continue;
                }
                waiting_on_attempt = true;
                wait_for_attempt(wait.as_mut().expect("queue wait exists"), deadline).await?;
            }
            BindingStart::Bound(binding) => return Ok(binding.target().clone()),
        }
    }
}

fn create_attempting_session(
    registry: &Arc<AffinityRegistry>,
    route_id: ModelRouteId,
    raw: &str,
) -> crate::affinity::BindingLease {
    let mut lease = match registry
        .begin_session(ProtocolDialect::OpenAiResponses, route_id, raw, TTL)
        .expect("begin session")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("session must have one creator: {other:?}"),
    };
    lease.mark_attempting().expect("promote session creator");
    lease
}

fn spawn_waiter(
    registry: Arc<AffinityRegistry>,
    queue: Arc<QueueCoordinator>,
    route_id: ModelRouteId,
    raw: &'static str,
) -> tokio::task::JoinHandle<Result<AffinityTarget, any2api_domain::PublicError>> {
    tokio::spawn(async move {
        wait_for_session_binding(&registry, &queue, route_id, raw, WAIT_TIMEOUT, 1).await
    })
}

async fn wait_until_waiting(queue: &QueueCoordinator, expected: u32) {
    for _ in 0..10_000 {
        if queue.waiting_count() == expected {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("queue waiter did not start");
}

fn target(route_id: ModelRouteId) -> AffinityTarget {
    AffinityTarget::new(
        route_id,
        RouteTargetId::new(),
        CredentialId::new().into(),
        "upstream-model",
        ProtocolDialect::OpenAiResponses,
        ProtocolDialect::OpenAiResponses,
    )
}
