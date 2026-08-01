use std::{sync::Arc, time::Duration};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect, PublicErrorCode, RouteTargetId};

use super::{SessionBindingStart, acquire_session_binding};
use crate::{
    affinity::{AffinityRegistry, AffinityTarget, BindingStart},
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
    let _creator = create_session(&registry, route_id, "queue-full");

    let error = acquire_session_binding(
        &registry,
        &queue,
        ProtocolDialect::OpenAiResponses,
        route_id,
        "queue-full",
        TTL,
        WAIT_TIMEOUT,
        1,
    )
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
    let _creator = create_session(&registry, route_id, "timeout");
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
    let _creator = create_session(&registry, route_id, "cancel");
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
    let first_creator = create_session(&registry, route_id, "recreated");
    let task = spawn_waiter(
        Arc::clone(&registry),
        Arc::clone(&queue),
        route_id,
        "recreated",
    );

    wait_until_waiting(&queue, 1).await;
    tokio::time::advance(Duration::from_millis(500)).await;
    drop(first_creator);
    let _second_creator = create_session(&registry, route_id, "recreated");
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
    let creator = create_session(&registry, route_id, "deadline-commit");
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

fn create_session(
    registry: &Arc<AffinityRegistry>,
    route_id: ModelRouteId,
    raw: &str,
) -> crate::affinity::BindingLease {
    match registry
        .begin_session(ProtocolDialect::OpenAiResponses, route_id, raw, TTL)
        .expect("begin session")
    {
        BindingStart::Create(lease) => lease,
        other => panic!("session must have one creator: {other:?}"),
    }
}

fn spawn_waiter(
    registry: Arc<AffinityRegistry>,
    queue: Arc<QueueCoordinator>,
    route_id: ModelRouteId,
    raw: &'static str,
) -> tokio::task::JoinHandle<Result<SessionBindingStart, any2api_domain::PublicError>> {
    tokio::spawn(async move {
        acquire_session_binding(
            &registry,
            &queue,
            ProtocolDialect::OpenAiResponses,
            route_id,
            raw,
            TTL,
            WAIT_TIMEOUT,
            1,
        )
        .await
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
