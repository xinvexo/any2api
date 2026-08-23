use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

use any2api_domain::{GatewayApiKeyConfiguration, GatewayApiKeyId, RequestsPerMinute};

const RATE_WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GatewayApiKeyRateLimitExceeded {
    retry_after_seconds: u64,
}

impl GatewayApiKeyRateLimitExceeded {
    #[must_use]
    pub const fn retry_after_seconds(self) -> u64 {
        self.retry_after_seconds
    }
}

#[derive(Debug, Default)]
pub(crate) struct GatewayApiKeyRateRegistry {
    handles: RwLock<HashMap<GatewayApiKeyId, Arc<GatewayApiKeyRateHandle>>>,
}

impl GatewayApiKeyRateRegistry {
    pub(crate) fn reconcile(
        &self,
        configuration: &GatewayApiKeyConfiguration,
    ) -> GatewayApiKeyRateBindings {
        self.reconcile_specs(
            configuration
                .keys()
                .iter()
                .map(|key| (key.id(), key.requests_per_minute())),
        )
    }

    fn reconcile_specs(
        &self,
        specs: impl IntoIterator<Item = (GatewayApiKeyId, Option<RequestsPerMinute>)>,
    ) -> GatewayApiKeyRateBindings {
        let mut handles = self
            .handles
            .write()
            .expect("Gateway API Key rate registry lock poisoned");
        let mut active_ids = HashSet::new();
        let mut bindings = HashMap::new();

        for (id, requests_per_minute) in specs {
            active_ids.insert(id);
            let handle = handles
                .entry(id)
                .or_insert_with(|| Arc::new(GatewayApiKeyRateHandle::default()));
            bindings.insert(
                id,
                GatewayApiKeyRateBinding {
                    handle: Arc::clone(handle),
                    requests_per_minute,
                },
            );
        }

        handles.retain(|id, _| active_ids.contains(id));
        GatewayApiKeyRateBindings { bindings }
    }
}

#[derive(Debug)]
pub(crate) struct GatewayApiKeyRateBindings {
    bindings: HashMap<GatewayApiKeyId, GatewayApiKeyRateBinding>,
}

impl GatewayApiKeyRateBindings {
    pub(crate) fn try_admit(
        &self,
        id: GatewayApiKeyId,
    ) -> Result<(), GatewayApiKeyRateLimitExceeded> {
        self.bindings
            .get(&id)
            .expect("authenticated Gateway API Key has a runtime rate binding")
            .try_admit()
    }
}

#[derive(Clone, Debug)]
struct GatewayApiKeyRateBinding {
    handle: Arc<GatewayApiKeyRateHandle>,
    requests_per_minute: Option<RequestsPerMinute>,
}

impl GatewayApiKeyRateBinding {
    fn try_admit(&self) -> Result<(), GatewayApiKeyRateLimitExceeded> {
        self.try_admit_at(Instant::now())
    }

    fn try_admit_at(&self, now: Instant) -> Result<(), GatewayApiKeyRateLimitExceeded> {
        let Some(limit) = self.requests_per_minute else {
            return Ok(());
        };
        self.handle.try_admit(now, limit)
    }
}

#[derive(Debug, Default)]
struct GatewayApiKeyRateHandle {
    attempts: Mutex<VecDeque<Instant>>,
}

impl GatewayApiKeyRateHandle {
    fn try_admit(
        &self,
        now: Instant,
        limit: RequestsPerMinute,
    ) -> Result<(), GatewayApiKeyRateLimitExceeded> {
        let mut attempts = self
            .attempts
            .lock()
            .expect("Gateway API Key rate window lock poisoned");
        let cutoff = now.checked_sub(RATE_WINDOW);
        while attempts
            .front()
            .is_some_and(|attempt| cutoff.is_some_and(|cutoff| *attempt <= cutoff))
        {
            attempts.pop_front();
        }

        let limit = limit.get() as usize;
        if attempts.len() >= limit {
            let retry_at = attempts[attempts.len() - limit] + RATE_WINDOW;
            let retry_after = retry_at.saturating_duration_since(now);
            let retry_after_seconds = retry_after
                .as_secs()
                .saturating_add(u64::from(retry_after.subsec_nanos() != 0))
                .max(1);
            return Err(GatewayApiKeyRateLimitExceeded {
                retry_after_seconds,
            });
        }

        attempts.push_back(now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Barrier},
        thread,
        time::{Duration, Instant},
    };

    use any2api_domain::{GatewayApiKeyId, RequestsPerMinute};

    use super::GatewayApiKeyRateRegistry;

    #[test]
    fn concurrent_admission_never_exceeds_the_configured_limit() {
        let registry = GatewayApiKeyRateRegistry::default();
        let id = GatewayApiKeyId::new();
        let bindings = registry.reconcile_specs([(id, Some(rpm(5)))]);
        let binding = bindings.bindings.get(&id).expect("binding").clone();
        let barrier = Arc::new(Barrier::new(32));
        let now = Instant::now();

        let admitted = thread::scope(|scope| {
            let workers = (0..32)
                .map(|_| {
                    let binding = binding.clone();
                    let barrier = Arc::clone(&barrier);
                    scope.spawn(move || {
                        barrier.wait();
                        binding.try_admit_at(now).is_ok()
                    })
                })
                .collect::<Vec<_>>();
            workers
                .into_iter()
                .map(|worker| worker.join().expect("worker"))
                .filter(|admitted| *admitted)
                .count()
        });

        assert_eq!(admitted, 5);
    }

    #[test]
    fn reconciling_a_lower_limit_preserves_the_existing_window() {
        let registry = GatewayApiKeyRateRegistry::default();
        let id = GatewayApiKeyId::new();
        let first = registry.reconcile_specs([(id, Some(rpm(3)))]);
        let first = first.bindings.get(&id).expect("first binding");
        let start = Instant::now();
        first.try_admit_at(start).expect("first request");
        first
            .try_admit_at(start + Duration::from_secs(1))
            .expect("second request");
        first
            .try_admit_at(start + Duration::from_secs(2))
            .expect("third request");

        let updated = registry.reconcile_specs([(id, Some(rpm(1)))]);
        let limited = updated
            .bindings
            .get(&id)
            .expect("updated binding")
            .try_admit_at(start + Duration::from_secs(3))
            .expect_err("the existing requests still fill the lowered window");

        assert_eq!(limited.retry_after_seconds(), 59);
    }

    #[test]
    fn deleting_a_key_releases_the_registry_binding_but_not_old_snapshots() {
        let registry = GatewayApiKeyRateRegistry::default();
        let id = GatewayApiKeyId::new();
        let old = registry.reconcile_specs([(id, Some(rpm(1)))]);
        let start = Instant::now();
        old.bindings
            .get(&id)
            .expect("old binding")
            .try_admit_at(start)
            .expect("first request");

        registry.reconcile_specs([]);
        let recreated = registry.reconcile_specs([(id, Some(rpm(1)))]);
        recreated
            .bindings
            .get(&id)
            .expect("new binding")
            .try_admit_at(start)
            .expect("recreated id has a new runtime");
        assert!(
            old.bindings
                .get(&id)
                .expect("old snapshot binding")
                .try_admit_at(start)
                .is_err()
        );
    }

    fn rpm(value: u32) -> RequestsPerMinute {
        RequestsPerMinute::new(value).expect("valid RPM")
    }
}
