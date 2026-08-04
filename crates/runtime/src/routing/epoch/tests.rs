use std::{sync::Arc, time::Duration};

use tokio::time::Instant;

use super::SchedulerEpoch;
use crate::lifecycle::ProcessLifecycle;

#[tokio::test(start_paused = true)]
async fn duplicate_and_concurrent_schedules_use_one_worker() {
    let lifecycle = ProcessLifecycle::new();
    let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
    let slots = (0..32).map(|_| epoch.wake_slot()).collect::<Vec<_>>();
    let deadline = Instant::now() + Duration::from_secs(10);
    slots[0].schedule(deadline);

    std::thread::scope(|scope| {
        for slot in &slots {
            scope.spawn(|| {
                for _ in 0..100 {
                    slot.schedule(deadline);
                }
            });
        }
    });

    assert_eq!(lifecycle.background_task_count(), 1);
    assert_eq!(epoch.scheduled_wake_count(), slots.len());
    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(epoch.current(), 1);
    assert_eq!(epoch.scheduled_wake_count(), 0);
    assert_eq!(lifecycle.background_task_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn deferred_schedule_updates_the_index_before_publishing_the_worker_notification() {
    let lifecycle = ProcessLifecycle::new();
    let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
    let slot = epoch.wake_slot();
    let deadline = Instant::now() + Duration::from_secs(10);

    let notification = slot.prepare_schedule(deadline);
    assert_eq!(epoch.scheduled_wake_count(), 1);
    assert_eq!(lifecycle.background_task_count(), 0);

    notification.publish();
    assert_eq!(lifecycle.background_task_count(), 1);
    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(epoch.current(), 1);
}

#[tokio::test(start_paused = true)]
async fn deferred_cancellation_updates_the_index_before_notifying_the_worker() {
    let lifecycle = ProcessLifecycle::new();
    let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
    let slot = epoch.wake_slot();
    slot.schedule(Instant::now() + Duration::from_secs(10));
    tokio::task::yield_now().await;

    let notification = slot.prepare_cancellation();
    assert_eq!(epoch.scheduled_wake_count(), 0);
    notification.publish();

    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(epoch.current(), 0);
    assert_eq!(lifecycle.background_task_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn an_earlier_replacement_reorders_the_worker_sleep() {
    let lifecycle = ProcessLifecycle::new();
    let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
    let slot = epoch.wake_slot();
    slot.schedule(Instant::now() + Duration::from_secs(60));
    tokio::task::yield_now().await;

    slot.schedule(Instant::now() + Duration::from_secs(10));
    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;

    assert_eq!(epoch.current(), 1);
    assert_eq!(epoch.scheduled_wake_count(), 0);
    assert_eq!(lifecycle.background_task_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_later_replacement_cancels_the_old_deadline() {
    let lifecycle = ProcessLifecycle::new();
    let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
    let slot = epoch.wake_slot();
    slot.schedule(Instant::now() + Duration::from_secs(10));
    tokio::task::yield_now().await;

    slot.schedule(Instant::now() + Duration::from_secs(30));
    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(epoch.current(), 0);
    assert_eq!(epoch.scheduled_wake_count(), 1);

    tokio::time::advance(Duration::from_secs(20)).await;
    tokio::task::yield_now().await;
    assert_eq!(epoch.current(), 1);
    assert_eq!(epoch.scheduled_wake_count(), 0);
    assert_eq!(lifecycle.background_task_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn later_slots_are_retained_after_the_earliest_wake() {
    let lifecycle = ProcessLifecycle::new();
    let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
    let first = epoch.wake_slot();
    let second = epoch.wake_slot();
    first.schedule(Instant::now() + Duration::from_secs(10));
    second.schedule(Instant::now() + Duration::from_secs(20));

    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(epoch.current(), 1);
    assert_eq!(epoch.scheduled_wake_count(), 1);

    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(epoch.current(), 2);
    assert_eq!(epoch.scheduled_wake_count(), 0);
    assert_eq!(lifecycle.background_task_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn equal_deadlines_are_coalesced_into_one_epoch_advance() {
    let lifecycle = ProcessLifecycle::new();
    let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
    let first = epoch.wake_slot();
    let second = epoch.wake_slot();
    let deadline = Instant::now() + Duration::from_secs(10);
    first.schedule(deadline);
    second.schedule(deadline);

    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(epoch.current(), 1);
    assert_eq!(epoch.scheduled_wake_count(), 0);
    assert_eq!(lifecycle.background_task_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn dropping_a_slot_cancels_its_pending_wake() {
    let lifecycle = ProcessLifecycle::new();
    let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
    let slot = epoch.wake_slot();
    slot.schedule(Instant::now() + Duration::from_secs(10));
    drop(slot);

    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;
    assert_eq!(epoch.current(), 0);
    assert_eq!(epoch.scheduled_wake_count(), 0);
    assert_eq!(lifecycle.background_task_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn worker_exits_at_draining_and_cannot_restart() {
    let lifecycle = ProcessLifecycle::new();
    let epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
    let slot = epoch.wake_slot();
    slot.schedule(Instant::now() + Duration::from_secs(60));
    assert_eq!(lifecycle.background_task_count(), 1);

    lifecycle.begin_draining();
    lifecycle.close_background_tasks();
    lifecycle.wait_for_background_tasks().await;
    assert_eq!(lifecycle.background_task_count(), 0);

    let after_draining = epoch.wake_slot();
    after_draining.schedule(Instant::now() + Duration::from_secs(1));
    tokio::time::advance(Duration::from_secs(60)).await;
    assert_eq!(epoch.current(), 0);
    assert_eq!(lifecycle.background_task_count(), 0);
}

#[test]
fn scheduler_can_exist_without_a_tokio_runtime_until_a_wake_is_needed() {
    let epoch = SchedulerEpoch::new();
    let _slot = epoch.wake_slot();
    assert_eq!(epoch.current(), 0);
    assert_eq!(epoch.scheduled_wake_count(), 0);
    assert_eq!(Arc::strong_count(&epoch), 2);
}
