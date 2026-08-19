use any2api_memory_reclaimer::mark_current_thread_as_mimalloc_pool_worker;

pub(super) fn install(builder: &mut tokio::runtime::Builder) {
    builder.on_thread_start(mark_current_thread_as_mimalloc_pool_worker);
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
