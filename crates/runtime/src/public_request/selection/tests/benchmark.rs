use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use super::super::{filter_recorder::RequestFilterRecorder, generation};
use super::candidate;
use crate::routing::{
    QueueCoordinator, QueuePolicy, RateLimitAction, RouteCandidate, SchedulerEpoch,
};

const SAMPLE_COUNT: usize = 3;
// Cover the default 128-waiter policy, deliberately large candidate sets,
// a large queue override, and the accepted 100,000-waiter setting maximum.
const CASES: [(usize, usize); 7] = [
    (1, 1),
    (128, 32),
    (128, 1_024),
    (128, 10_000),
    (4_096, 32),
    (4_096, 1_024),
    (100_000, 1),
];

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "manual scheduler epoch herd scaling benchmark"]
async fn scheduler_epoch_herd_scaling_benchmark() {
    for (waiter_count, candidate_count) in CASES {
        let mut samples = Vec::with_capacity(SAMPLE_COUNT);
        for _ in 0..SAMPLE_COUNT {
            samples.push(benchmark_case(waiter_count, candidate_count).await);
        }
        samples.sort_unstable();
        let elapsed = samples[SAMPLE_COUNT / 2];
        let candidate_checks = waiter_count.saturating_mul(candidate_count);
        eprintln!(
            "scheduler epoch herd benchmark: waiters={waiter_count}, candidates={candidate_count}, \
             candidate_checks={candidate_checks}, median={elapsed:?}, ns_per_check={}",
            elapsed.as_nanos() / candidate_checks.max(1) as u128,
        );
    }
}

async fn benchmark_case(waiter_count: usize, candidate_count: usize) -> Duration {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let tiers = Arc::new(BTreeMap::from([(
        0,
        rate_limited_candidates(candidate_count, &epoch),
    )]));
    let completed_selections = Arc::new(AtomicUsize::new(0));
    let policy = QueuePolicy::new(
        RateLimitAction::Wait,
        Duration::from_secs(300),
        u32::try_from(waiter_count).expect("benchmark waiter count fits u32"),
        false,
    )
    .expect("benchmark queue policy");
    let mut tasks = Vec::with_capacity(waiter_count);
    for _ in 0..waiter_count {
        let coordinator = Arc::clone(&coordinator);
        let tiers = Arc::clone(&tiers);
        let completed_selections = Arc::clone(&completed_selections);
        tasks.push(tokio::spawn(async move {
            let mut filters = RequestFilterRecorder::default();
            generation::select_with_queue(&coordinator, policy, || {
                let result = generation::try_select_for_test_with_recorder(
                    false,
                    &tiers,
                    &mut filters,
                    |_| Some(0),
                );
                completed_selections.fetch_add(1, Ordering::AcqRel);
                result
            })
            .await
        }));
    }

    wait_for_at_least(
        || coordinator.waiting_count() as usize,
        waiter_count,
        "queued waiters",
    )
    .await;
    wait_for_at_least(
        || completed_selections.load(Ordering::Acquire),
        waiter_count * 2,
        "completed initial selections",
    )
    .await;
    let baseline = completed_selections.load(Ordering::Acquire);
    let started = Instant::now();
    epoch.advance();
    wait_for_at_least(
        || completed_selections.load(Ordering::Acquire),
        baseline + waiter_count,
        "completed epoch reselections",
    )
    .await;
    let elapsed = started.elapsed();
    for task in &tasks {
        task.abort();
    }
    for task in tasks {
        let _ = task.await;
    }
    assert_eq!(coordinator.waiting_count(), 0);
    elapsed
}

fn rate_limited_candidates(
    candidate_count: usize,
    epoch: &Arc<SchedulerEpoch>,
) -> Vec<RouteCandidate> {
    (0..candidate_count)
        .map(|index| {
            let candidate = candidate(
                &format!("epoch-benchmark-{index}"),
                (index % 256) as u8,
                Arc::clone(epoch),
                0,
            );
            drop(candidate.binding.try_reserve().expect("exhaust RPM"));
            candidate
        })
        .collect()
}

async fn wait_for_at_least(
    mut current: impl FnMut() -> usize,
    expected: usize,
    phase: &'static str,
) {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if current() >= expected {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {phase}");
        tokio::task::yield_now().await;
    }
}
