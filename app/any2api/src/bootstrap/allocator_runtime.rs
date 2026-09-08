use std::{
    cell::Cell,
    time::{Duration, Instant},
};

use any2api_memory_reclaimer::{
    collect_current_thread, mark_current_thread_as_mimalloc_pool_worker,
};

const WORKER_RECLAIM_INTERVAL: Duration = Duration::from_secs(30);

thread_local! {
    static LAST_RECLAIM: Cell<Option<Instant>> = const { Cell::new(None) };
}

pub(super) fn install(builder: &mut tokio::runtime::Builder) {
    builder.on_thread_start(mark_current_thread_as_mimalloc_pool_worker);
    builder.on_thread_park(reclaim_idle_worker);
}

fn reclaim_idle_worker() {
    LAST_RECLAIM.with(|last| {
        let now = Instant::now();
        if last
            .get()
            .is_some_and(|previous| now.duration_since(previous) < WORKER_RECLAIM_INTERVAL)
        {
            return;
        }
        last.set(Some(now));
        collect_current_thread();
    });
}

#[cfg(test)]
mod tests {
    use super::install;
    use std::time::Duration;

    #[test]
    fn hooks_run_on_scheduler_and_blocking_pool_threads() {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder.enable_all().worker_threads(2);
        install(&mut builder);
        let runtime = builder.build().expect("runtime should start");

        runtime.block_on(async {
            tokio::spawn(async {})
                .await
                .expect("scheduler task should run");
            tokio::task::spawn_blocking(|| {})
                .await
                .expect("blocking task should run");
        });
        runtime.shutdown_timeout(Duration::from_secs(1));
    }
}
