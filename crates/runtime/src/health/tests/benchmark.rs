use std::{
    sync::{Arc, Barrier, Mutex},
    thread,
    time::{Duration, Instant as WallInstant},
};

use tokio::time::Instant;

use crate::routing::SchedulerEpoch;

const SAMPLE_COUNT: usize = 20_000;

#[derive(Clone, Copy)]
enum NotificationPosition {
    Held,
    Deferred,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "manual health wake notification contention benchmark"]
async fn health_wake_notification_contention_benchmark() {
    let epoch = SchedulerEpoch::new();
    let held = benchmark_position(&epoch, NotificationPosition::Held).await;
    let deferred = benchmark_position(&epoch, NotificationPosition::Deferred).await;

    eprintln!(
        "health wake notification contention benchmark: samples={SAMPLE_COUNT}, \
         held_wait_median={:?}, held_wait_p99={:?}, \
         deferred_wait_median={:?}, deferred_wait_p99={:?}, \
         notification_median={:?}, notification_p99={:?}",
        percentile(&held.lock_waits, 50),
        percentile(&held.lock_waits, 99),
        percentile(&deferred.lock_waits, 50),
        percentile(&deferred.lock_waits, 99),
        percentile(&held.notifications, 50),
        percentile(&held.notifications, 99),
    );
}

struct BenchmarkSamples {
    lock_waits: Vec<Duration>,
    notifications: Vec<Duration>,
}

async fn benchmark_position(
    epoch: &Arc<SchedulerEpoch>,
    position: NotificationPosition,
) -> BenchmarkSamples {
    let wake = Arc::new(epoch.wake_slot());
    let initial_deadline = Instant::now() + Duration::from_secs(3_600);
    wake.schedule(initial_deadline);
    tokio::task::yield_now().await;

    let health_lock = Arc::new(Mutex::new(()));
    let start = Arc::new(Barrier::new(2));
    let contend = Arc::new(Barrier::new(2));
    let finish = Arc::new(Barrier::new(2));
    let writer = {
        let health_lock = Arc::clone(&health_lock);
        let start = Arc::clone(&start);
        let contend = Arc::clone(&contend);
        let finish = Arc::clone(&finish);
        let wake = Arc::clone(&wake);
        thread::spawn(move || {
            let mut notifications = Vec::with_capacity(SAMPLE_COUNT);
            for sample in 0..SAMPLE_COUNT {
                let health_guard = health_lock.lock().expect("benchmark health lock poisoned");
                start.wait();
                contend.wait();
                thread::yield_now();
                let offset = u64::try_from(sample + 1).expect("sample index fits u64");
                let notification_started = WallInstant::now();
                match position {
                    NotificationPosition::Held => {
                        wake.schedule(initial_deadline + Duration::from_nanos(offset));
                        drop(health_guard);
                    }
                    NotificationPosition::Deferred => {
                        let notification =
                            wake.prepare_schedule(initial_deadline + Duration::from_nanos(offset));
                        drop(health_guard);
                        notification.publish();
                    }
                }
                notifications.push(notification_started.elapsed());
                finish.wait();
            }
            notifications
        })
    };
    let reader = thread::spawn(move || {
        let mut samples = Vec::with_capacity(SAMPLE_COUNT);
        for _ in 0..SAMPLE_COUNT {
            start.wait();
            contend.wait();
            let started = WallInstant::now();
            let health_guard = health_lock.lock().expect("benchmark health lock poisoned");
            samples.push(started.elapsed());
            drop(health_guard);
            finish.wait();
        }
        samples
    });

    let mut notifications = writer.join().expect("benchmark writer");
    let mut lock_waits = reader.join().expect("benchmark reader");
    notifications.sort_unstable();
    lock_waits.sort_unstable();
    BenchmarkSamples {
        lock_waits,
        notifications,
    }
}

fn percentile(samples: &[Duration], percentile: usize) -> Duration {
    let index = (samples.len() - 1) * percentile / 100;
    samples[index]
}
