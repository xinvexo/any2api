use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use any2api_domain::ProviderKind;
use tokio::{
    sync::Mutex as AsyncMutex,
    time::{Instant, sleep_until},
};

pub(in crate::oauth) const OAUTH_CONTROL_PLANE_MIN_START_INTERVAL: Duration =
    Duration::from_millis(500);

pub(in crate::oauth) struct OAuthControlPlanePacer {
    interval: Duration,
    gates: Mutex<HashMap<ProviderKind, Arc<AsyncMutex<Instant>>>>,
}

impl OAuthControlPlanePacer {
    pub(in crate::oauth) fn new(interval: Duration) -> Self {
        Self {
            interval,
            gates: Mutex::new(HashMap::new()),
        }
    }

    pub(in crate::oauth) async fn wait(&self, provider: ProviderKind) {
        if self.interval.is_zero() {
            return;
        }
        let gate = {
            let mut gates = self
                .gates
                .lock()
                .expect("OAuth control-plane pacer lock poisoned");
            Arc::clone(
                gates
                    .entry(provider)
                    .or_insert_with(|| Arc::new(AsyncMutex::new(Instant::now()))),
            )
        };
        let mut next_start = gate.lock().await;
        let now = Instant::now();
        if *next_start > now {
            sleep_until(*next_start).await;
        }
        *next_start = Instant::now() + self.interval;
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use any2api_domain::ProviderKind;

    use super::OAuthControlPlanePacer;

    #[tokio::test(start_paused = true)]
    async fn same_provider_starts_are_spaced_but_other_providers_are_independent() {
        let interval = Duration::from_secs(1);
        let pacer = Arc::new(OAuthControlPlanePacer::new(interval));
        pacer.wait(ProviderKind::Codex).await;

        let waiting = {
            let pacer = Arc::clone(&pacer);
            tokio::spawn(async move { pacer.wait(ProviderKind::Codex).await })
        };
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());

        pacer.wait(ProviderKind::Claude).await;
        tokio::time::advance(interval - Duration::from_millis(1)).await;
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());

        tokio::time::advance(Duration::from_millis(1)).await;
        waiting.await.expect("paced request");
    }

    #[tokio::test(start_paused = true)]
    async fn cancelling_a_waiter_does_not_reserve_an_extra_start_slot() {
        let interval = Duration::from_secs(1);
        let pacer = Arc::new(OAuthControlPlanePacer::new(interval));
        pacer.wait(ProviderKind::Codex).await;

        let cancelled = {
            let pacer = Arc::clone(&pacer);
            tokio::spawn(async move { pacer.wait(ProviderKind::Codex).await })
        };
        tokio::task::yield_now().await;
        cancelled.abort();
        assert!(
            cancelled
                .await
                .expect_err("waiter must be cancelled")
                .is_cancelled()
        );

        let next = {
            let pacer = Arc::clone(&pacer);
            tokio::spawn(async move { pacer.wait(ProviderKind::Codex).await })
        };
        tokio::time::advance(interval).await;
        next.await.expect("next waiter uses the original slot");
    }
}
