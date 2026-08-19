use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use any2api_domain::OAuthAccountId;
use futures_util::{StreamExt, stream::FuturesUnordered};
use tokio::{sync::Notify, time::Instant};

use super::{coordinator::OAuthQuotaService, types::OAuthQuotaError};
use crate::lifecycle::ProcessLifecycle;

const ACTIVITY_DEBOUNCE: Duration = Duration::from_secs(5);
const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_STATE_RETENTION: Duration = Duration::from_secs(3_600);
const MAX_CONCURRENT_REFRESHES: usize = 6;

#[derive(Clone)]
pub(crate) struct OAuthQuotaActivity {
    shared: Arc<ActivityShared>,
}

struct ActivityShared {
    state: Mutex<ActivityState>,
    notify: Notify,
    started: AtomicBool,
}

#[derive(Default)]
struct ActivityState {
    accounts: HashMap<OAuthAccountId, AccountActivity>,
}

struct AccountActivity {
    due: Option<Instant>,
    in_flight: bool,
    dirty: bool,
    last_attempt: Option<Instant>,
}

impl OAuthQuotaActivity {
    pub(super) fn new() -> Self {
        Self {
            shared: Arc::new(ActivityShared {
                state: Mutex::new(ActivityState::default()),
                notify: Notify::new(),
                started: AtomicBool::new(false),
            }),
        }
    }

    pub(super) fn start(
        &self,
        service: Arc<OAuthQuotaService>,
        lifecycle: &ProcessLifecycle,
    ) -> bool {
        if self
            .shared
            .started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        lifecycle.spawn_until_draining(run(self.clone(), service, lifecycle.clone()));
        true
    }

    pub(crate) fn guard(&self, id: OAuthAccountId) -> OAuthQuotaActivityGuard {
        OAuthQuotaActivityGuard {
            activity: Some(self.clone()),
            id,
        }
    }

    fn record(&self, id: OAuthAccountId, now: Instant) {
        let mut state = self
            .shared
            .state
            .lock()
            .expect("OAuth quota activity state");
        prune_idle(&mut state, now);
        let account = state.accounts.entry(id).or_insert(AccountActivity {
            due: None,
            in_flight: false,
            dirty: false,
            last_attempt: None,
        });
        if account.in_flight {
            account.dirty = true;
        } else if account.due.is_none() {
            let debounce_due = now + ACTIVITY_DEBOUNCE;
            account.due = Some(account.last_attempt.map_or(debounce_due, |last| {
                (last + MIN_REFRESH_INTERVAL).max(debounce_due)
            }));
        }
        drop(state);
        self.shared.notify.notify_one();
    }

    fn take_due(&self, now: Instant, limit: usize) -> Vec<OAuthAccountId> {
        if limit == 0 {
            return Vec::new();
        }
        let mut state = self
            .shared
            .state
            .lock()
            .expect("OAuth quota activity state");
        let mut due = state
            .accounts
            .iter()
            .filter_map(|(id, activity)| {
                activity
                    .due
                    .filter(|due| *due <= now && !activity.in_flight)
                    .map(|due| (*id, due))
            })
            .collect::<Vec<_>>();
        due.sort_unstable_by_key(|(_, due)| *due);
        due.truncate(limit);
        for (id, _) in &due {
            let activity = state.accounts.get_mut(id).expect("due account exists");
            activity.due = None;
            activity.in_flight = true;
            activity.last_attempt = Some(now);
        }
        due.into_iter().map(|(id, _)| id).collect()
    }

    fn complete(&self, id: OAuthAccountId, now: Instant) {
        let mut state = self
            .shared
            .state
            .lock()
            .expect("OAuth quota activity state");
        let Some(activity) = state.accounts.get_mut(&id) else {
            return;
        };
        activity.in_flight = false;
        if activity.dirty {
            activity.dirty = false;
            activity.due = Some(
                activity
                    .last_attempt
                    .map_or(now, |last| last + MIN_REFRESH_INTERVAL)
                    .max(now),
            );
        }
        drop(state);
        self.shared.notify.notify_one();
    }

    fn next_due(&self) -> Option<Instant> {
        self.shared
            .state
            .lock()
            .expect("OAuth quota activity state")
            .accounts
            .values()
            .filter_map(|activity| activity.due)
            .min()
    }
}

pub(crate) struct OAuthQuotaActivityGuard {
    activity: Option<OAuthQuotaActivity>,
    id: OAuthAccountId,
}

impl Drop for OAuthQuotaActivityGuard {
    fn drop(&mut self) {
        if let Some(activity) = self.activity.take() {
            let now = Instant::now();
            activity.record(self.id, now);
        }
    }
}

type RefreshTask =
    Pin<Box<dyn Future<Output = (OAuthAccountId, Result<(), OAuthQuotaError>)> + Send + 'static>>;

async fn run(
    activity: OAuthQuotaActivity,
    service: Arc<OAuthQuotaService>,
    lifecycle: ProcessLifecycle,
) {
    let mut running = FuturesUnordered::<RefreshTask>::new();
    loop {
        let available = MAX_CONCURRENT_REFRESHES.saturating_sub(running.len());
        for id in activity.take_due(Instant::now(), available) {
            let service = Arc::clone(&service);
            running.push(Box::pin(async move {
                (id, service.refresh(id).await.map(|_| ()))
            }));
        }

        let next_due = activity.next_due();
        if running.is_empty() {
            match next_due {
                Some(due) => {
                    tokio::select! {
                        () = activity.shared.notify.notified() => {}
                        () = tokio::time::sleep_until(due) => {}
                    }
                }
                None => activity.shared.notify.notified().await,
            }
            continue;
        }

        let completed = match next_due {
            Some(due) => {
                tokio::select! {
                    completed = running.next() => completed,
                    () = activity.shared.notify.notified() => None,
                    () = tokio::time::sleep_until(due) => None,
                }
            }
            None => {
                tokio::select! {
                    completed = running.next() => completed,
                    () = activity.shared.notify.notified() => None,
                }
            }
        };
        if let Some((id, result)) = completed {
            if let Err(error) = result {
                tracing::warn!(oauth_account_id = %id, error = %error, "automatic OAuth quota refresh failed");
            }
            lifecycle.record_activity();
            activity.complete(id, Instant::now());
        }
    }
}

fn prune_idle(state: &mut ActivityState, now: Instant) {
    state.accounts.retain(|_, activity| {
        activity.in_flight
            || activity.due.is_some()
            || activity
                .last_attempt
                .is_none_or(|last| now.duration_since(last) < IDLE_STATE_RETENTION)
    });
}

#[cfg(test)]
mod tests;
